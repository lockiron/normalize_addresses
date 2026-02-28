use serde::{Deserialize, Serialize};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub lat: f64,
    pub lng: f64,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedAddress {
    pub pref: String,
    pub city: String,
    pub town: String,
    pub addr: String,
    pub level: u8,
    pub point: Option<Point>,
    pub other: String,
}

#[derive(Debug, Deserialize)]
pub struct TownItem {
    pub town: String,
    pub koaza: String,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
}

static PREFECTURES: [&str; 47] = [
    "北海道", "青森県", "岩手県", "宮城県", "秋田県", "山形県", "福島県", "茨城県",
    "栃木県", "群馬県", "埼玉県", "千葉県", "東京都", "神奈川県", "新潟県", "富山県",
    "石川県", "福井県", "山梨県", "長野県", "岐阜県", "静岡県", "愛知県", "三重県",
    "滋賀県", "京都府", "大阪府", "兵庫県", "奈良県", "和歌山県", "鳥取県", "島根県",
    "岡山県", "広島県", "山口県", "徳島県", "香川県", "愛媛県", "高知県", "福岡県",
    "佐賀県", "長崎県", "熊本県", "大分県", "宮崎県", "鹿児島県", "沖縄県",
];



pub async fn normalize_async(input: &str) -> anyhow::Result<NormalizedAddress> {
    let mut s = input.trim().to_string();

    // 1. Prefecture
    let mut pref = String::new();
    for p in PREFECTURES.iter() {
        if s.starts_with(p) {
            pref = p.to_string();
            s = s[p.len()..].trim().to_string();
            break;
        }
    }

    let url_ja = "https://geolonia.github.io/japanese-addresses/api/ja.json";
    let mut city = String::new();

    if pref.is_empty() {
        // Prefecture missing - infer from city
        if let Ok(res) = reqwest::get(url_ja).await {
            if res.status().is_success() {
                if let Ok(data) = res.json::<std::collections::HashMap<String, Vec<String>>>().await {
                    let mut candidates = vec![];
                    for (p, cities) in data.iter() {
                        let mut sorted_cities = cities.clone();
                        sorted_cities.sort_by(|a, b| b.len().cmp(&a.len()));
                        for c in sorted_cities {
                            if s.starts_with(&c) {
                                candidates.push((p.clone(), c.clone()));
                            }
                        }
                    }

                    if candidates.len() == 1 {
                        pref = candidates[0].0.clone();
                        city = candidates[0].1.clone();
                        s = s[city.len()..].trim().to_string();
                    } else if candidates.len() > 1 {
                        // Multiple cities match (e.g. "府中市") - try to resolve with town match
                        for (p, c) in candidates {
                            let url_towns = format!("https://geolonia.github.io/japanese-addresses/api/ja/{}/{}.json", p, c);
                            if let Ok(res_towns) = reqwest::get(&url_towns).await {
                                if res_towns.status().is_success() {
                                    if let Ok(towns) = res_towns.json::<Vec<TownItem>>().await {
                                        let s_after_city = s[c.len()..].trim().to_string();
                                        for t in towns {
                                            if let Some(pattern) = build_town_regex(&t.town) {
                                                if let Ok(re) = Regex::new(&pattern) {
                                                    if re.is_match(&s_after_city) {
                                                        pref = p;
                                                        city = c;
                                                        s = s_after_city;
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if !pref.is_empty() { break; }
                        }
                    }
                }
            }
        }
    }

    if pref.is_empty() {
        return Ok(NormalizedAddress {
            pref: "".to_string(),
            city: "".to_string(),
            town: "".to_string(),
            addr: s,
            level: 0,
            point: None,
            other: "".to_string(),
        });
    }

    if city.is_empty() {
        // 2. City (if not already inferred)
        if let Ok(res) = reqwest::get(url_ja).await {
            if res.status().is_success() {
                if let Ok(mut data) = res.json::<std::collections::HashMap<String, Vec<String>>>().await {
                    if let Some(mut cities) = data.remove(&pref) {
                        cities.sort_by(|a, b| b.len().cmp(&a.len()));
                        for c in cities {
                            if s.starts_with(&c) {
                                city = c.clone();
                                s = s[c.len()..].trim().to_string();
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if city.is_empty() {
        return Ok(NormalizedAddress {
            pref,
            city: "".to_string(),
            town: "".to_string(),
            addr: s,
            level: 1, // Prefecture level
            point: None,
            other: "".to_string(),
        });
    }

    // 3. Fetch town data
    let url = format!("https://geolonia.github.io/japanese-addresses/api/ja/{}/{}.json", pref, city);
    let response = reqwest::get(&url).await;

    let mut towns: Vec<TownItem> = vec![];
    if let Ok(res) = response {
        if res.status().is_success() {
            if let Ok(data) = res.json::<Vec<TownItem>>().await {
                towns = data;
            }
        }
    }

    // 4. Town match
    // Sort towns by descending length to match longest first
    towns.sort_by(|a, b| b.town.len().cmp(&a.town.len()));

    let mut matched_town = None;
    let mut remaining = s.clone();

    for t in towns {
        if let Some(pattern) = build_town_regex(&t.town) {
            if let Ok(re) = Regex::new(&pattern) {
                if let Some(caps) = re.captures(&s) {
                    if let Some(m) = caps.get(0) {
                        remaining = s[m.end()..].trim().to_string();
                        let mut point = None;
                        if let (Some(lat), Some(lng)) = (t.lat, t.lng) {
                            point = Some(Point { lat, lng, level: 8 });
                        }
                        matched_town = Some((t.town, point));
                        break;
                    }
                }
            }
        }
    }

    if let Some((t, point)) = matched_town {
        let level = if point.is_some() { 8 } else { 3 };
        
        // Strip leading hyphen from remaining addr if present
        let addr = if remaining.starts_with('-') {
            remaining[1..].to_string()
        } else {
            remaining
        };
        
        return Ok(NormalizedAddress {
            pref,
            city,
            town: t,
            addr,
            level,
            point,
            other: "".to_string(),
        });
    }

    // No town match
    let level = if city.is_empty() { 1 } else { 3 };
    Ok(NormalizedAddress {
        pref,
        city,
        town: "".to_string(),
        addr: s,
        level,
        point: None,
        other: "".to_string(),
    })
}

fn kanji_to_arabic(s: &str) -> String {
    // Basic kanji to arabic conversion 
    // e.g., "二十四" -> "24", "一" -> "1"
    let mut val = 0;
    let mut current_unit = 0;
    let mut temp = 0;
    
    for ch in s.chars() {
        match ch {
            '一' | '壱' => temp = 1,
            '二' => temp = 2,
            '三' => temp = 3,
            '四' => temp = 4,
            '五' => temp = 5,
            '六' => temp = 6,
            '七' => temp = 7,
            '八' => temp = 8,
            '九' => temp = 9,
            '十' => {
                if temp == 0 { temp = 1; }
                current_unit += temp * 10;
                temp = 0;
            }
            '百' => {
                if temp == 0 { temp = 1; }
                current_unit += temp * 100;
                temp = 0;
            }
            '千' => {
                if temp == 0 { temp = 1; }
                current_unit += temp * 1000;
                temp = 0;
            }
            _ => {}
        }
    }
    val += current_unit + temp;
    
    if val > 0 || String::from(s).contains('零') || String::from(s).contains('〇') {
        val.to_string()
    } else {
        s.to_string()
    }
}

fn build_town_regex(town: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"([壱一二三四五六七八九十百千]+)([丁町]目?|番[町丁]|条|軒|線|[のノ]町|地割|号)").unwrap());
    
    let mut pattern = "^".to_string();
    let mut last_idx = 0;
    
    for caps in re.captures_iter(town) {
        if let Some(m) = caps.get(0) {
            // Add prefix text securely escaped
            pattern.push_str(&regex::escape(&town[last_idx..m.start()]));
            
            let num_kanji = caps.get(1).unwrap().as_str();
            let suffix = caps.get(2).unwrap().as_str();
            
            let arabic = kanji_to_arabic(num_kanji);
            
            // Build regex group matching Kanji OR Arabic mapped number
            let group = format!("({}|{})", regex::escape(num_kanji), arabic);
            
            // Allow suffix OR a hyphen
            let suffix_group = format!("({}|-)", regex::escape(suffix));
            
            pattern.push_str(&group);
            pattern.push_str(&suffix_group);
            
            last_idx = m.end();
        }
    }
    
    pattern.push_str(&regex::escape(&town[last_idx..]));
    
    // In Geolonia's logic, we should probably allow an optional hyphen after the town?
    // Not necessarily required since we just want to match the prefix string
    Some(pattern)
}

pub fn convert_digits_to_kanji(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let mut num_str = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                num_str.push(chars[i]);
                i += 1;
            }
            if let Ok(num) = num_str.parse::<u32>() {
                result.push_str(&number_to_kanji(num));
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    
    result
}

fn number_to_kanji(mut n: u32) -> String {
    if n == 0 {
        return "零".to_string();
    }
    
    let mut result = String::new();
    
    let man = n / 10000;
    n %= 10000;
    if man > 0 {
        result.push_str(&number_to_kanji_10000(man));
        result.push('万');
    }
    
    if n > 0 {
        result.push_str(&number_to_kanji_10000(n));
    }
    
    result
}

fn number_to_kanji_10000(mut n: u32) -> String {
    let digits = [' ', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    let units = [' ', '十', '百', '千'];
    
    let mut result = String::new();
    let mut div = 1000;
    for i in (0..4).rev() {
        let d = (n / div) as usize;
        n %= div;
        div /= 10;
        
        if d > 0 {
            if d > 1 || i == 0 {
                result.push(digits[d]);
            }
            if i > 0 {
                result.push(units[i]);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanji_conversion() {
        assert_eq!(convert_digits_to_kanji("24-2-2"), "二十四-二-二");
        assert_eq!(number_to_kanji(24), "二十四");
        assert_eq!(number_to_kanji(1), "一");
        assert_eq!(number_to_kanji(12), "十二");
        assert_eq!(number_to_kanji(20), "二十");
        assert_eq!(number_to_kanji(100), "百");
        assert_eq!(number_to_kanji(102), "百二");
        assert_eq!(number_to_kanji(123), "百二十三");
        assert_eq!(number_to_kanji(1234), "千二百三十四");
    }

    #[test]
    fn test_find_original_len() {
        assert_eq!(find_original_len("24軒2条", "二十四軒二条"), "24軒2条".len());
        assert_eq!(find_original_len("24-2-2-3-3", "二十四-二-二"), "24-2-2".len());
    }
}
