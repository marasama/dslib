use crate::dslib::plotter::describe::process_column;
use memmap2::Mmap;
use num_traits::Float;
use std::{fmt::Display, fs::File};

/// Sütun odaklı (columnar) veri saklama yapısı.
/// Veriler satır satır değil, doğrudan bellek bitişikliği (data locality)
/// ve CPU önbellek (cache) verimliliği sağlayan sütun vektörlerinde saklanır.
#[derive(Debug)]
pub enum Column<F: Float> {
    Text(Vec<String>),
    Number {
        rows: Vec<F>,
        count: usize,
        mean: F,
        std: F,
        min: F,
        per_25: F,
        per_50: F,
        per_75: F,
        max: F,
        range: F,
    },
}

/// Ayrıştırılan CSV verisinin nihai çıktısı.
#[derive(Debug)]
pub struct CsvData<F: Float> {
    pub headers: Vec<String>,
    pub columns: Vec<Column<F>>,
    pub row_count: usize,
}

/// Ayrıştırma sırasında karşılaşılabilecek olası sözdizimi hataları.
#[derive(Debug)]
pub enum CsvError {
    /// Tırnakla başlayan bir alanın kapatılmadan dosyanın bitmesi durumu.
    UnterminatedQuote { position: usize },
    /// Tırnaksız bir alanın ortasında geçersiz tırnak karakteri bulunması.
    QuoteInUnquotedField { position: usize },
    /// Bir satırdaki sütun sayısının beklenen sütun sayısıyla uyuşmaması.
    RowLengthMismatch {
        row: usize,
        expected: usize,
        found: usize,
    },
}

impl Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsvError::UnterminatedQuote { position } => {
                write!(
                    f,
                    "Kapatılmamış tırnaklı alan! Başlangıç bayt indeksi: {position}"
                )
            }
            CsvError::QuoteInUnquotedField { position } => {
                write!(
                    f,
                    "Tırnaksız alan içerisinde beklenmeyen tırnak karakteri! Bayt: {position}"
                )
            }
            CsvError::RowLengthMismatch {
                row,
                expected,
                found,
            } => {
                write!(
                    f,
                    "Satır {row} üzerinde {found} sütun bulundu, beklenen: {expected}!"
                )
            }
        }
    }
}

impl std::error::Error for CsvError {}

/// Sonlu Durum Makinesi (FSM) durumları.
#[derive(Debug, PartialEq, Eq)]
enum ReadingState {
    FieldStart,    // Yeni bir hücre başlangıcı
    InField,       // Tırnaksız düz metin hücresi içi
    InQuoteField,  // Tırnak içine alınmış hücre içi
    QuoteInQuoted, // Tırnaklı alanda tırnak karakteri görüldükten hemen sonraki durum ("" kaçışı veya kapanış)
}

/// Sıfır Kopya (Zero-Copy) Hücre Temsili.
/// Standart metinler için bellekte yeni bir String tahsis edilmez; doğrudan
/// mmap üzerindeki bayt dilimi (`Borrowed(&'a [u8])`) taşınır.
/// Sadece içinde `""` kaçışı (escape) bulunan hücreler dinamik buffer (`Owned(Vec<u8>)`) gerektirir.
enum FieldRef<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

impl<'a> FieldRef<'a> {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        match self {
            FieldRef::Borrowed(s) => s,
            FieldRef::Owned(v) => v.as_slice(),
        }
    }

    /// Yalnızca metin olarak kalacağı kesinleşen sütunlar için heap üzerinde String üretir.
    fn to_string_lossy(&self) -> String {
        match self {
            FieldRef::Borrowed(s) => String::from_utf8_lossy(s).into_owned(),
            FieldRef::Owned(v) => String::from_utf8_lossy(v).into_owned(),
        }
    }
}

/// Tip çıkarımı tamamlandıktan sonra sütunların doğrudan yazıldığı tampon yapı.
enum ColumnBuffer<F: Float> {
    Number(Vec<F>),
    Text(Vec<String>),
}

/// Windows formatındaki CRLF (\r\n) veya tekil CR/LF geçişlerini tüketir.
#[inline(always)]
fn consume_crlf(input: &[u8], i: usize) -> usize {
    if i + 1 < input.len() && input[i + 1] == b'\n' {
        2
    } else {
        1
    }
}

/// Bayt diliminin başındaki ve sonundaki ASCII boşluklarını kopyalamadan kırpar (trim).
#[inline(always)]
fn trim_ascii(mut b: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = b.split_first() {
        if first.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    while let Some((&last, rest)) = b.split_last() {
        if last.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    b
}

/// Bayt dilimini ara String tahsisi yapmadan doğrudan float tipine dönüştürür.
/// Boş hücreler `NaN` olarak değerlendirilir.
#[inline]
fn parse_float<F: Float>(bytes: &[u8]) -> Option<F> {
    let trimmed = trim_ascii(bytes);
    if trimmed.is_empty() {
        Some(F::nan())
    } else {
        let s = std::str::from_utf8(trimmed).ok()?;
        F::from_str_radix(s, 10).ok()
    }
}

/// 1. AŞAMA (PASS 1): Ön Tarama (Dry Run).
/// Bellekte HİÇBİR veri tahsisatı (allocation) yapmadan dosyayı baştan sona tarar:
/// - Toplam satır sayısını (`row_count`) ve sütun sayısını (`col_count`) hesaplar.
/// - CSV sözdizimsel bozukluklarını (kapanmamış tırnak, satır uzunluğu uyuşmazlığı)
///   ana parse adımına geçmeden ve bellek ayırmadan erken safhada yakalar.
fn scan_dimensions(mmap: &[u8]) -> Result<(usize, usize), CsvError> {
    let len = mmap.len();
    if len == 0 {
        return Ok((0, 0));
    }

    let mut state = ReadingState::FieldStart;
    let mut row_count = 0usize;
    let mut col_count = 0usize;
    let mut current_cols = 0usize;
    let mut quote_start_pos = 0usize;
    let mut i = 0usize;

    while i < len {
        let b = mmap[i];
        match state {
            ReadingState::FieldStart => {
                if b == b'"' {
                    state = ReadingState::InQuoteField;
                    quote_start_pos = i;
                    i += 1;
                } else if b == b',' {
                    current_cols += 1;
                    i += 1;
                } else if b == b'\n' || b == b'\r' {
                    current_cols += 1;
                    if row_count == 0 {
                        col_count = current_cols;
                    } else if current_cols != col_count {
                        return Err(CsvError::RowLengthMismatch {
                            row: row_count,
                            expected: col_count,
                            found: current_cols,
                        });
                    }
                    row_count += 1;
                    current_cols = 0;
                    i += if b == b'\r' { consume_crlf(mmap, i) } else { 1 };
                } else {
                    state = ReadingState::InField;
                    i += 1;
                }
            }
            ReadingState::InField => {
                // Ayraçlara kadar olan güvenli baytları tek bir iç döngüyle hızlıca atla (skip-loop)
                while i < len && !matches!(mmap[i], b',' | b'\n' | b'\r' | b'"') {
                    i += 1;
                }
                if i >= len {
                    break;
                }
                let byte = mmap[i];
                if byte == b',' {
                    current_cols += 1;
                    state = ReadingState::FieldStart;
                    i += 1;
                } else if byte == b'\n' || byte == b'\r' {
                    current_cols += 1;
                    if row_count == 0 {
                        col_count = current_cols;
                    } else if current_cols != col_count {
                        return Err(CsvError::RowLengthMismatch {
                            row: row_count,
                            expected: col_count,
                            found: current_cols,
                        });
                    }
                    row_count += 1;
                    current_cols = 0;
                    state = ReadingState::FieldStart;
                    i += if byte == b'\r' {
                        consume_crlf(mmap, i)
                    } else {
                        1
                    };
                } else if byte == b'"' {
                    return Err(CsvError::QuoteInUnquotedField { position: i });
                }
            }
            ReadingState::InQuoteField => {
                while i < len && mmap[i] != b'"' {
                    i += 1;
                }
                if i >= len {
                    break;
                }
                state = ReadingState::QuoteInQuoted;
                i += 1;
            }
            ReadingState::QuoteInQuoted => {
                if b == b'"' {
                    // "" kaçışı: tırnak içi okumaya devam et
                    state = ReadingState::InQuoteField;
                    i += 1;
                } else if b == b',' {
                    current_cols += 1;
                    state = ReadingState::FieldStart;
                    i += 1;
                } else if b == b'\n' || b == b'\r' {
                    current_cols += 1;
                    if row_count == 0 {
                        col_count = current_cols;
                    } else if current_cols != col_count {
                        return Err(CsvError::RowLengthMismatch {
                            row: row_count,
                            expected: col_count,
                            found: current_cols,
                        });
                    }
                    row_count += 1;
                    current_cols = 0;
                    state = ReadingState::FieldStart;
                    i += if b == b'\r' { consume_crlf(mmap, i) } else { 1 };
                } else {
                    state = ReadingState::InField;
                }
            }
        }
    }

    // Dosya sonundaki (EOF) kalan son alanın doğrulaması
    match state {
        ReadingState::InQuoteField => {
            return Err(CsvError::UnterminatedQuote {
                position: quote_start_pos,
            });
        }
        ReadingState::FieldStart => {
            if current_cols > 0 {
                current_cols += 1;
                if row_count == 0 {
                    col_count = current_cols;
                } else if current_cols != col_count {
                    return Err(CsvError::RowLengthMismatch {
                        row: row_count,
                        expected: col_count,
                        found: current_cols,
                    });
                }
                row_count += 1;
            }
        }
        ReadingState::InField | ReadingState::QuoteInQuoted => {
            current_cols += 1;
            if row_count == 0 {
                col_count = current_cols;
            } else if current_cols != col_count {
                return Err(CsvError::RowLengthMismatch {
                    row: row_count,
                    expected: col_count,
                    found: current_cols,
                });
            }
            row_count += 1;
        }
    }

    Ok((row_count, col_count))
}

/// Ayrıştırma sırasında sütun verilerini ve başlıkları toplayan yardımcı yapı.
struct CsvBuilder<'a, F: Float> {
    headers: Vec<String>,
    columns: Option<Vec<ColumnBuffer<F>>>,
    sample_rows: Vec<Vec<FieldRef<'a>>>,
    current_row_buffer: Vec<FieldRef<'a>>,
    current_col: usize,
    col_count: usize,
    data_rows: usize,
    set_headers: bool,
}

impl<'a, F: Float + Display> CsvBuilder<'a, F> {
    fn new(col_count: usize, data_rows: usize, set_headers: bool) -> Self {
        Self {
            headers: Vec::with_capacity(if set_headers { col_count } else { 0 }),
            columns: None,
            sample_rows: Vec::with_capacity(5.min(data_rows)),
            current_row_buffer: Vec::with_capacity(col_count),
            current_col: 0,
            col_count,
            data_rows,
            set_headers,
        }
    }

    fn push_field(&mut self, field: FieldRef<'a>) {
        if self.set_headers && self.headers.len() < self.col_count {
            self.headers.push(field.to_string_lossy());
            self.current_col += 1;
            if self.current_col == self.col_count {
                self.current_col = 0;
            }
            return;
        }

        if let Some(col_bufs) = self.columns.as_mut() {
            match &mut col_bufs[self.current_col] {
                ColumnBuffer::Number(vec) => {
                    if let Some(num) = parse_float::<F>(field.as_bytes()) {
                        vec.push(num);
                    } else {
                        // GÜVENLİK DÜŞÜRMESİ (Demotion Fallback):
                        // İlk 5 satırda sayı gelip daha sonra beklenmeyen bir metin çıkarsa;
                        // o sütun bozulmadan metne (Text) dönüştürülür ve hafıza korunur.
                        let mut text_vec = Vec::with_capacity(self.data_rows);
                        for num in vec.drain(..) {
                            if num.is_nan() {
                                text_vec.push(String::new());
                            } else {
                                text_vec.push(num.to_string());
                            }
                        }
                        text_vec.push(field.to_string_lossy());
                        col_bufs[self.current_col] = ColumnBuffer::Text(text_vec);
                    }
                }
                ColumnBuffer::Text(vec) => {
                    vec.push(field.to_string_lossy());
                }
            }
            self.current_col += 1;
            if self.current_col == self.col_count {
                self.current_col = 0;
            }
        } else {
            self.current_row_buffer.push(field);
            if self.current_row_buffer.len() == self.col_count {
                self.sample_rows.push(std::mem::replace(
                    &mut self.current_row_buffer,
                    Vec::with_capacity(self.col_count),
                ));

                if self.sample_rows.len() == 5 || self.sample_rows.len() == self.data_rows {
                    let mut col_bufs: Vec<ColumnBuffer<F>> = (0..self.col_count)
                        .map(|c| {
                            // Sütunun tüm örnek satırları sayıya dönüştürülebiliyor mu?
                            // Sadece en az bir adet geçerli sayı içeren ve tüm dolu hücreleri sayı olan sütunlar Number sayılır.
                            let has_numeric_value = self.sample_rows.iter().any(|row| {
                                let trimmed = trim_ascii(row[c].as_bytes());
                                !trimmed.is_empty() && parse_float::<F>(trimmed).is_some()
                            });
                            let all_non_empty_are_numeric = self.sample_rows.iter().all(|row| {
                                let trimmed = trim_ascii(row[c].as_bytes());
                                trimmed.is_empty() || parse_float::<F>(trimmed).is_some()
                            });
                            let is_numeric = has_numeric_value && all_non_empty_are_numeric;

                            if is_numeric {
                                ColumnBuffer::Number(Vec::with_capacity(self.data_rows))
                            } else {
                                ColumnBuffer::Text(Vec::with_capacity(self.data_rows))
                            }
                        })
                        .collect();

                    for row in self.sample_rows.drain(..) {
                        for (c, f) in row.into_iter().enumerate() {
                            match &mut col_bufs[c] {
                                ColumnBuffer::Number(vec) => {
                                    vec.push(parse_float::<F>(f.as_bytes()).unwrap_or_else(F::nan));
                                }
                                ColumnBuffer::Text(vec) => {
                                    vec.push(f.to_string_lossy());
                                }
                            }
                        }
                    }
                    self.columns = Some(col_bufs);
                }
            }
        }
    }

    fn finish(mut self) -> (Vec<String>, Vec<Column<F>>) {
        if self.current_col > 0 {
            self.push_field(FieldRef::Borrowed(&[]));
        }

        let final_columns = self
            .columns
            .unwrap_or_else(|| {
                (0..self.col_count)
                    .map(|_| ColumnBuffer::Text(Vec::new()))
                    .collect()
            })
            .into_iter()
            .map(|col| match col {
                ColumnBuffer::Number(vec) => process_column(vec),
                ColumnBuffer::Text(vec) => Column::Text(vec),
            })
            .collect();

        (self.headers, final_columns)
    }
}

/// 2. AŞAMA (PASS 2): Ana Ayrıştırma ve Tip Çıkarımı.
pub fn parse_csv<'a, F: Float + Display>(
    mmap: &'a [u8],
    set_headers: bool,
) -> Result<CsvData<F>, CsvError> {
    // Adım 1: Satır ve sütun boyutlarını önceden al
    let (total_rows, col_count) = scan_dimensions(mmap)?;

    if total_rows == 0 || col_count == 0 {
        return Ok(CsvData {
            headers: Vec::new(),
            columns: Vec::new(),
            row_count: 0,
        });
    }

    let data_rows = if set_headers {
        total_rows.saturating_sub(1)
    } else {
        total_rows
    };

    let len = mmap.len();
    let mut builder = CsvBuilder::<F>::new(col_count, data_rows, set_headers);

    let mut state = ReadingState::FieldStart;
    let mut field_start = 0usize;
    let mut quote_start_pos = 0usize;
    let mut escaped_buf: Option<Vec<u8>> = None;
    let mut i = 0usize;

    // İkinci geçiş: Bayt akışı üzerinden hücre dilimlerini ayıkla
    while i < len {
        match state {
            ReadingState::FieldStart => {
                let b = mmap[i];
                if b == b'"' {
                    state = ReadingState::InQuoteField;
                    quote_start_pos = i;
                    field_start = i + 1;
                    escaped_buf = None;
                    i += 1;
                } else if b == b',' {
                    builder.push_field(FieldRef::Borrowed(&[]));
                    i += 1;
                } else if b == b'\n' {
                    builder.push_field(FieldRef::Borrowed(&[]));
                    i += 1;
                } else if b == b'\r' {
                    builder.push_field(FieldRef::Borrowed(&[]));
                    i += consume_crlf(mmap, i);
                } else {
                    field_start = i;
                    state = ReadingState::InField;
                    escaped_buf = None;
                    i += 1;
                }
            }
            ReadingState::InField => {
                while i < len && !matches!(mmap[i], b',' | b'\n' | b'\r' | b'"') {
                    i += 1;
                }
                if i >= len {
                    break;
                }
                let b = mmap[i];
                if b == b',' || b == b'\n' || b == b'\r' {
                    let slice = &mmap[field_start..i];
                    let field = match escaped_buf.take() {
                        Some(mut buf) => {
                            buf.extend_from_slice(slice);
                            FieldRef::Owned(buf)
                        }
                        None => FieldRef::Borrowed(slice),
                    };
                    builder.push_field(field);
                    state = ReadingState::FieldStart;
                    i += if b == b'\r' { consume_crlf(mmap, i) } else { 1 };
                } else if b == b'"' {
                    return Err(CsvError::QuoteInUnquotedField { position: i });
                }
            }
            ReadingState::InQuoteField => {
                let chunk_start = i;
                while i < len && mmap[i] != b'"' {
                    i += 1;
                }
                if let Some(ref mut buf) = escaped_buf {
                    buf.extend_from_slice(&mmap[chunk_start..i]);
                }
                if i >= len {
                    break;
                }
                state = ReadingState::QuoteInQuoted;
                i += 1;
            }
            ReadingState::QuoteInQuoted => {
                let b = mmap[i];
                if b == b'"' {
                    // Escape edilmiş tırnak: hafızada Owned tampon oluşturup içine tek tırnak ekle
                    if escaped_buf.is_none() {
                        let mut buf = Vec::with_capacity((i - field_start) * 2);
                        buf.extend_from_slice(&mmap[field_start..i - 1]);
                        buf.push(b'"');
                        escaped_buf = Some(buf);
                    } else {
                        escaped_buf.as_mut().unwrap().push(b'"');
                    }
                    state = ReadingState::InQuoteField;
                    i += 1;
                } else if b == b',' || b == b'\n' || b == b'\r' {
                    let field = match escaped_buf.take() {
                        Some(buf) => FieldRef::Owned(buf),
                        None => FieldRef::Borrowed(&mmap[field_start..i - 1]),
                    };
                    builder.push_field(field);
                    state = ReadingState::FieldStart;
                    i += if b == b'\r' { consume_crlf(mmap, i) } else { 1 };
                } else {
                    if escaped_buf.is_none() {
                        let mut buf = Vec::new();
                        buf.extend_from_slice(&mmap[field_start..i - 1]);
                        escaped_buf = Some(buf);
                    }
                    field_start = i;
                    state = ReadingState::InField;
                }
            }
        }
    }

    // Dosya sonlandığında açıkta kalan son hücreyi sütuna aktar
    match state {
        ReadingState::InField => {
            let slice = &mmap[field_start..len];
            let field = match escaped_buf.take() {
                Some(mut buf) => {
                    buf.extend_from_slice(slice);
                    FieldRef::Owned(buf)
                }
                None => FieldRef::Borrowed(slice),
            };
            builder.push_field(field);
        }
        ReadingState::QuoteInQuoted => {
            let field = match escaped_buf.take() {
                Some(buf) => FieldRef::Owned(buf),
                None => FieldRef::Borrowed(&mmap[field_start..len.saturating_sub(1)]),
            };
            builder.push_field(field);
        }
        ReadingState::InQuoteField => {
            return Err(CsvError::UnterminatedQuote {
                position: quote_start_pos,
            });
        }
        ReadingState::FieldStart => {}
    }

    let (headers, columns) = builder.finish();

    Ok(CsvData {
        headers,
        columns,
        row_count: data_rows,
    })
}

/// Dosyayı işletim sistemi seviyesinde doğrudan sanal belleğe bağlar (mmap)
/// ve diskten belleğe ek bir kopyalama yapmadan ayrıştırmayı tetikler.
pub fn read_csv<F: Float + Display>(
    file_name: &str,
    set_headers: bool,
) -> std::io::Result<CsvData<F>> {
    let file = File::open(file_name)?;
    let mmap = unsafe { Mmap::map(&file)? };

    parse_csv(&mmap, set_headers)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
