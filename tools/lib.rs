pub fn text_normalize(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(|c| match c {
            'ك' => Some('ک'),
            'ؤ' => Some('و'),
            'ي' | 'ئ' => Some('ی'),
            '۰' | '٠' => Some('0'),
            '۱' | '١' => Some('1'),
            '۲' | '٢' => Some('2'),
            '۳' | '٣' => Some('3'),
            '۴' | '٤' => Some('4'),
            '۵' | '٥' => Some('5'),
            '۶' | '٦' => Some('6'),
            '۷' | '٧' => Some('7'),
            '۸' | '٨' => Some('8'),
            '۹' | '٩' => Some('9'),
            'أ' | 'إ' | 'آ' | 'ٱ' => Some('ا'),
            '\u{200C}' => Some(' '),
            '\u{064B}'..='\u{065F}' => None,
            _ => Some(c),
        })
        .collect()
}
