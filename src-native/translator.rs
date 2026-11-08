use std::time::Duration;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TranslationResult {
    pub text: String,
    pub engine: String,
    pub fallback_from: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EngineBenchmark {
    pub id: String,
    pub name: String,
    pub latency_ms: u64,
    pub sample: String,
    pub success: bool,
    pub error: Option<String>,
}

pub fn parse_target_tag(text: &str, default_target: &str) -> (String, String) {
    let re = match regex::Regex::new(r"<([a-zA-Z]{2}(-[a-zA-Z]{2})?)\b") {
        Ok(r) => r,
        Err(_) => return (default_target.to_string(), text.to_string()),
    };

    if let Some(cap) = re.captures(text) {
        if let Some(m) = cap.get(0) {
            let mut code = cap.get(1).map(|c| c.as_str().to_lowercase()).unwrap_or_default();
            if code == "cn" {
                code = "zh-CN".to_string();
            } else if code == "tw" {
                code = "zh-TW".to_string();
            }
            let clean = text.replace(m.as_str(), "");
            let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
            return (code, clean);
        }
    }
    (default_target.to_string(), text.to_string())
}

fn unescape_html(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn get_rfc7231_date() -> String {
    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let mins = (day_secs % 3600) / 60;
    let sec = day_secs % 60;

    let day_of_week = match (days + 4) % 7 {
        0 => "Sun", 1 => "Mon", 2 => "Tue", 3 => "Wed", 4 => "Thu", 5 => "Fri", _ => "Sat",
    };

    let mut year = 1970;
    let mut rem_days = days;
    loop {
        let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let year_days = if is_leap { 366 } else { 365 };
        if rem_days >= year_days {
            rem_days -= year_days;
            year += 1;
        } else {
            break;
        }
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days = [
        31, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut month_idx = 0;
    for (i, &mdays) in month_days.iter().enumerate() {
        if rem_days >= mdays {
            rem_days -= mdays;
        } else {
            month_idx = i;
            break;
        }
    }
    let mday = rem_days + 1;

    format!("{day_of_week}, {:02} {} {:04} {:02}:{:02}:{:02} GMT", mday, months[month_idx], year, hours, mins, sec)
}

fn generate_guid() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:032x}", t)
}

fn get_microsoft_signature(url: &str, date_str: &str, guid: &str) -> Result<String, String> {
    let private_key: [u8; 64] = [
        0xa2, 0x29, 0x3a, 0x3d, 0xd0, 0xdd, 0x32, 0x73, 0x97, 0x7a, 0x64, 0xdb, 0xc2, 0xf3,
        0x27, 0xf5, 0xd7, 0xbf, 0x87, 0xd9, 0x45, 0x9d, 0xf0, 0x5a, 0x09, 0x66, 0xc6, 0x30,
        0xc6, 0x6a, 0xaa, 0x84, 0x9a, 0x41, 0xaa, 0x94, 0x3a, 0xa8, 0xd5, 0x1a, 0x6e, 0x4d,
        0xaa, 0xc9, 0xa3, 0x70, 0x12, 0x35, 0xc7, 0xeb, 0x12, 0xf6, 0xe8, 0x23, 0x07, 0x9e,
        0x47, 0x10, 0x95, 0x91, 0x88, 0x55, 0xd8, 0x17,
    ];
    let escaped_url: String = url
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect();

    let bytes_str = format!("MSTranslatorAndroidApp{}{}{}", escaped_url, date_str, guid).to_lowercase();
    let mut mac = HmacSha256::new_from_slice(&private_key).map_err(|e| e.to_string())?;
    mac.update(bytes_str.as_bytes());
    let result = mac.finalize().into_bytes();
    let b64 = base64::prelude::BASE64_STANDARD.encode(result);
    Ok(format!("MSTranslatorAndroidApp::{}::{}::{}", b64, date_str, guid))
}

/// 1. Google Translate Free (GTX endpoint)
pub fn translate_google_gtx(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let url = "https://translate.googleapis.com/translate_a/single";
    let resp = client
        .post(url)
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target),
            ("dt", "t"),
        ])
        .form(&[("q", text)])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Google GTX send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Google GTX HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Google GTX parse error: {e}"))?;
    let mut out = String::new();
    if let Some(arr) = json.get(0).and_then(|v| v.as_array()) {
        for seg in arr {
            if let Some(s) = seg.get(0).and_then(|v| v.as_str()) {
                out.push_str(s);
            }
        }
    }
    if out.trim().is_empty() {
        return Err("Google GTX empty result".into());
    }
    Ok(out)
}

/// 2. Google Translate Chrome Internal API (from LunaTranslator google.py)
pub fn translate_google_pa(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let url = "https://translate-pa.googleapis.com/v1/translateHtml";
    let body = serde_json::json!([
        [[text], "auto", target],
        "wt_lib"
    ]);

    let resp = client
        .post(url)
        .header("X-Goog-Api-Key", "AIzaSyATBXajvzQLTDHEQbcpq0Ihe0vWDHmO520")
        .header("Content-Type", "application/json+protobuf")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .json(&body)
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Google PA send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Google PA HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Google PA parse error: {e}"))?;
    if let Some(trans) = json.get(0).and_then(|v| v.get(0)).and_then(|v| v.as_str()) {
        let clean = unescape_html(trans);
        if !clean.trim().is_empty() {
            return Ok(clean);
        }
    }
    Err("Google PA empty result".into())
}

/// 3. Microsoft Translator (Cognitive Services with Mobile App Signature from LunaTranslator microsoft.py)
pub fn translate_microsoft(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let lang = match target {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        other => other,
    };
    let url_path = format!("api.cognitive.microsofttranslator.com/translate?api-version=3.0&to={}", lang);
    let date_str = get_rfc7231_date();
    let guid = generate_guid();
    let sig = get_microsoft_signature(&url_path, &date_str, &guid)?;

    let post_url = format!("https://{}", url_path);
    let body = serde_json::json!([{"Text": text}]);

    let resp = client
        .post(&post_url)
        .header("X-MT-Signature", sig)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Dalvik/2.1.0 (Linux; U; Android 14; Pixel 8 Build/UQ1A.240205.004)")
        .json(&body)
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Microsoft send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Microsoft HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Microsoft parse error: {e}"))?;
    if let Some(trans) = json.get(0)
        .and_then(|v| v.get("translations"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }
    Err("Microsoft empty result".into())
}

/// 4. Bing Web Translator (from LunaTranslator bing.py)
pub fn translate_bing(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let lang = match target {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        other => other,
    };

    let host_resp = client
        .get("https://www.bing.com/Translator")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Bing host fetch error: {e}"))?
        .text()
        .map_err(|e| e.to_string())?;

    let re_iid = regex::Regex::new(r#"data-iid="([^"]+)""#).map_err(|e| e.to_string())?;
    let re_ig = regex::Regex::new(r#"IG:"([^"]+)""#).map_err(|e| e.to_string())?;
    let re_params = regex::Regex::new(r#"var params_AbusePreventionHelper = (\[[^;]+\]);"#).map_err(|e| e.to_string())?;

    let iid = re_iid.captures(&host_resp).and_then(|c| c.get(1)).map(|m| m.as_str()).ok_or("Bing IID not found")?;
    let ig = re_ig.captures(&host_resp).and_then(|c| c.get(1)).map(|m| m.as_str()).ok_or("Bing IG not found")?;
    let params_str = re_params.captures(&host_resp).and_then(|c| c.get(1)).map(|m| m.as_str()).ok_or("Bing params not found")?;

    let parsed: serde_json::Value = serde_json::from_str(params_str).map_err(|e| e.to_string())?;
    let key = parsed.get(0).map(|v| v.to_string()).ok_or("Bing key not found")?;
    let token = parsed.get(1).and_then(|v| v.as_str()).ok_or("Bing token not found")?;

    let post_url = format!("https://www.bing.com/ttranslatev3?isVertical=1&IG={}&IID={}", ig, iid);
    let resp = client
        .post(&post_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .form(&[
            ("text", text),
            ("fromLang", "auto-detect"),
            ("to", lang),
            ("key", key.trim_matches('"')),
            ("token", token),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Bing send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Bing HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Bing parse error: {e}"))?;
    if let Some(trans) = json.get(0)
        .and_then(|v| v.get("translations"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }
    Err("Bing empty result".into())
}

/// 5. Yandex Browser Translator (from LunaTranslator yandex.py)
pub fn translate_yandex(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let tgt = if target.starts_with("zh") { "zh" } else { target };

    // 1. Detect source language
    let detect_url = "https://translate.yandex.net/api/v1/tr.json/detect";
    let detect_resp = client
        .get(detect_url)
        .query(&[
            ("srv", "browser_video_translation"),
            ("text", text),
        ])
        .timeout(Duration::from_secs(4))
        .send();

    let src = match detect_resp {
        Ok(r) if r.status().is_success() => {
            let json: serde_json::Value = r.json().unwrap_or_default();
            json.get("lang").and_then(|v| v.as_str()).unwrap_or("en").to_string()
        }
        _ => "en".to_string(),
    };

    let url = "https://browser.translate.yandex.net/api/v1/tr.json/translate";
    let lang_pair = format!("{}-{}", src, tgt);
    let resp = client
        .post(url)
        .query(&[
            ("srv", "browser_video_translation"),
            ("lang", &lang_pair),
            ("text", text),
        ])
        .form(&[("maxRetryCount", "2"), ("fetchAbortTimeout", "500")])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Yandex send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Yandex HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Yandex parse error: {e}"))?;
    if let Some(trans) = json.get("text").and_then(|v| v.get(0)).and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }
    Err("Yandex empty result".into())
}

/// 6. Papago (from LunaTranslator papago.py)
pub fn translate_papago(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let lang = match target {
        "zh-CN" => "zh-CN",
        "zh-TW" => "zh-TW",
        other => other,
    };
    let url = "https://papago.naver.com/api/text/translation";

    let resp = client
        .post(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .form(&[
            ("dict", "true"),
            ("dictDisplay", "30"),
            ("honorific", "false"),
            ("useGlossary", "false"),
            ("source", "auto"),
            ("target", lang),
            ("text", text),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Papago send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Papago HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Papago parse error: {e}"))?;
    if let Some(trans) = json.get("translatedText").and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }
    Err("Papago empty result".into())
}

/// 7. Translate.com (from LunaTranslator TranslateCom.py)
pub fn translate_translate_com(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let lang = if target.starts_with("zh") { "zh-TW" } else { target };

    // 1. Detect source language first because translate.com fails when source is "auto"
    let detect_url = "https://www.translate.com/translator/ajax_lang_auto_detect";
    let detect_resp = client
        .post(detect_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .form(&[("text_to_translate", text)])
        .timeout(Duration::from_secs(4))
        .send();

    let src = match detect_resp {
        Ok(r) if r.status().is_success() => {
            let json: serde_json::Value = r.json().unwrap_or_default();
            json.get("language").and_then(|v| v.as_str()).unwrap_or("en").to_string()
        }
        _ => "en".to_string(),
    };

    let url = "https://www.translate.com/translator/translate_mt";
    let resp = client
        .post(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .form(&[
            ("text_to_translate", text),
            ("source_lang", &src),
            ("translated_lang", lang),
            ("use_cache_only", "false"),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("Translate.com send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Translate.com HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("Translate.com parse error: {e}"))?;
    if let Some(trans) = json.get("translated_text").and_then(|v| v.as_str()) {
        let clean = unescape_html(trans);
        if !clean.trim().is_empty() {
            return Ok(clean);
        }
    }
    Err("Translate.com returned empty translation".into())
}

/// 8. MyMemory Free API
pub fn translate_mymemory(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let url = "https://api.mymemory.translated.net/get";
    let resp = client
        .get(url)
        .query(&[
            ("q", text),
            ("langpair", &format!("auto|{}", target)),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("MyMemory send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("MyMemory HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("MyMemory parse error: {e}"))?;
    if let Some(trans) = json.get("responseData").and_then(|v| v.get("translatedText")).and_then(|v| v.as_str()) {
        return Ok(unescape_html(trans));
    }
    Err("MyMemory empty result".into())
}

/// 9. DeepL (Web JSON-RPC endpoint with fallback)
pub fn translate_deepl(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    let tgt = match target {
        "zh-CN" => "ZH-HANS",
        "zh-TW" => "ZH-HANT",
        other => other,
    }.to_uppercase();

    let count_i = text.chars().filter(|c| *c == 'i').count();
    let num: u64 = 8350000;
    let req_id = num * 1000;
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let ts = if count_i != 0 {
        let i = (count_i + 1) as u64;
        now_ms - (now_ms % i) + i
    } else {
        now_ms
    };

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "LMT_handle_jobs",
        "id": req_id,
        "params": {
            "commonJobParams": {
                "formality": "undefined",
                "wasSpoken": false,
                "regionalVariant": "en-US",
                "transcribe_as": "romanize",
                "mode": "translate",
                "textType": "plaintext",
                "advancedMode": false
            },
            "lang": {
                "source_lang_user_selected": "auto",
                "target_lang": tgt,
                "source_lang_computed": "EN"
            },
            "jobs": [
                {
                    "sentences": [{"prefix": "", "text": text, "id": 0}],
                    "kind": "default",
                    "raw_en_context_after": [],
                    "raw_en_context_before": [],
                    "preferred_num_beams": 1
                }
            ],
            "timestamp": ts,
        }
    });

    let resp = client
        .post("https://www2.deepl.com/jsonrpc")
        .header("content-type", "application/json")
        .header("user-agent", "DeepL/1627620 CFNetwork/3826.500.62.2.1 Darwin/24.4.0")
        .header("accept", "*/*")
        .header("x-app-os-name", "iOS")
        .header("x-app-os-version", "18.4.0")
        .header("accept-language", "en-US,en;q=0.9")
        .header("x-app-device", "iPhone16,2")
        .header("referer", "https://www.deepl.com/")
        .header("x-product", "translator")
        .header("x-app-build", "1627620")
        .header("x-app-version", "25.1")
        .json(&body)
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("DeepL send error: {e}"))?;

    if !resp.status().is_success() {
        if resp.status().as_u16() == 429 {
            return Err("DeepL 429: Web bị chặn IP (chỉ hoạt động với proxy hoặc DeepL Free API key)".into());
        }
        return Err(format!("DeepL HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("DeepL parse error: {e}"))?;
    if let Some(trans) = json.get("result")
        .and_then(|v| v.get("translations"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("beams"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("sentences"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }

    if let Some(trans) = json.get("result")
        .and_then(|v| v.get("texts"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str()) {
        return Ok(trans.to_string());
    }

    Err("DeepL empty result".into())
}

/// 10. ModernMT (from LunaTranslator ModernMt.py)
pub fn translate_modernmt(client: &reqwest::blocking::Client, text: &str, target: &str) -> Result<String, String> {
    use md5::{Md5, Digest};

    let tgt = match target {
        "zh-CN" => "zh-CN",
        "zh-TW" => "zh-TW",
        other => other,
    };

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let verify_src = format!("webkey_E3sTuMjpP8Jez49GcYpDVH7r#{}#{}", ts, text);
    let mut hasher = Md5::new();
    hasher.update(verify_src.as_bytes());
    let verify = format!("{:x}", hasher.finalize());

    let body = serde_json::json!({
        "q": text,
        "source": "",
        "target": tgt,
        "ts": ts,
        "verify": verify,
        "hints": "",
        "multiline": "true"
    });

    let resp = client
        .post("https://webapi.modernmt.com/translate")
        .header("Origin", "https://www.modernmt.com")
        .header("Referer", "https://www.modernmt.com/translate")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-HTTP-Method-Override", "GET")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/133.0.0.0 Safari/537.36")
        .json(&body)
        .timeout(Duration::from_secs(6))
        .send()
        .map_err(|e| format!("ModernMT send error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("ModernMT HTTP {}", resp.status()));
    }

    let json: serde_json::Value = resp.json().map_err(|e| format!("ModernMT parse error: {e}"))?;
    if let Some(trans) = json.get("data").and_then(|v| v.get("translation")).and_then(|v| v.as_str()) {
        let clean = unescape_html(trans);
        if !clean.trim().is_empty() {
            return Ok(clean);
        }
    }
    Err("ModernMT empty result".into())
}

/// Execute translation using a specific named engine
fn execute_engine(client: &reqwest::blocking::Client, engine: &str, text: &str, target: &str) -> Result<String, String> {
    match engine {
        "google" => translate_google_gtx(client, text, target),
        "google_pa" => translate_google_pa(client, text, target),
        "microsoft" => translate_microsoft(client, text, target),
        "modernmt" => translate_modernmt(client, text, target),
        "bing" => translate_bing(client, text, target),
        "yandex" => translate_yandex(client, text, target),
        "papago" => translate_papago(client, text, target),
        "translate_com" => translate_translate_com(client, text, target),
        "mymemory" => translate_mymemory(client, text, target),
        "deepl" => translate_deepl(client, text, target),
        _ => Err(format!("Unknown engine: {engine}")),
    }
}

/// All available free engines in preferred fallback priority order
pub const FREE_ENGINES_ORDER: &[&str] = &[
    "google_pa",
    "microsoft",
    "modernmt",
    "bing",
    "google",
    "papago",
    "translate_com",
    "yandex",
    "mymemory",
    "deepl",
];

/// Translates with automatic fallback to other free engines if the primary engine fails.
/// Only falls back for that single translation request without persisting any configuration change.
pub fn translate_with_fallback(text: &str, target: &str, primary: &str) -> Result<TranslationResult, String> {
    if text.trim().is_empty() {
        return Ok(TranslationResult {
            text: String::new(),
            engine: primary.to_string(),
            fallback_from: None,
        });
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;

    // 1. Try primary engine first
    match execute_engine(&client, primary, text, target) {
        Ok(translated) if !translated.trim().is_empty() => {
            return Ok(TranslationResult {
                text: translated,
                engine: primary.to_string(),
                fallback_from: None,
            });
        }
        Err(err) => {
            eprintln!("[QuickTran] Primary engine '{}' failed: {}. Attempting fallback...", primary, err);
        }
        _ => {}
    }

    // 2. Iterate through other available engines in priority order
    let mut last_error = format!("Primary engine '{}' failed", primary);
    for &candidate in FREE_ENGINES_ORDER {
        if candidate == primary {
            continue;
        }

        match execute_engine(&client, candidate, text, target) {
            Ok(translated) if !translated.trim().is_empty() => {
                eprintln!("[QuickTran] Fallback succeeded with engine '{}'", candidate);
                return Ok(TranslationResult {
                    text: translated,
                    engine: candidate.to_string(),
                    fallback_from: Some(primary.to_string()),
                });
            }
            Err(e) => {
                eprintln!("[QuickTran] Candidate '{}' failed: {}", candidate, e);
                last_error = e;
            }
            _ => {}
        }
    }

    Err(format!("All translation engines failed. Last error: {}", last_error))
}

pub fn benchmark_single_engine(engine: &str, text: &str, target: &str) -> EngineBenchmark {
    let name = match engine {
        "google_pa" => "Google Chrome PA",
        "microsoft" => "Microsoft Translator",
        "modernmt" => "ModernMT",
        "bing" => "Bing Translator",
        "google" => "Google Translate (Web GTX)",
        "papago" => "Naver Papago",
        "yandex" => "Yandex Translator",
        "translate_com" => "Translate.com",
        "mymemory" => "MyMemory",
        "deepl" => "DeepL (Web / Free)",
        _ => engine,
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return EngineBenchmark {
                id: engine.to_string(),
                name: name.to_string(),
                latency_ms: 0,
                sample: String::new(),
                success: false,
                error: Some(e.to_string()),
            }
        }
    };

    let start = std::time::Instant::now();
    match execute_engine(&client, engine, text, target) {
        Ok(translated) if !translated.trim().is_empty() => {
            let elapsed = start.elapsed().as_millis() as u64;
            EngineBenchmark {
                id: engine.to_string(),
                name: name.to_string(),
                latency_ms: elapsed,
                sample: translated,
                success: true,
                error: None,
            }
        }
        Ok(_) => {
            let elapsed = start.elapsed().as_millis() as u64;
            EngineBenchmark {
                id: engine.to_string(),
                name: name.to_string(),
                latency_ms: elapsed,
                sample: String::new(),
                success: false,
                error: Some("Nội dung dịch rỗng".into()),
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            EngineBenchmark {
                id: engine.to_string(),
                name: name.to_string(),
                latency_ms: elapsed,
                sample: String::new(),
                success: false,
                error: Some(e),
            }
        }
    }
}

pub fn benchmark_all_engines(text: &str, target: &str) -> Vec<EngineBenchmark> {
    use std::sync::{Arc, Mutex};
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for &eng in FREE_ENGINES_ORDER {
        let res_clone = Arc::clone(&results);
        let t = text.to_string();
        let tg = target.to_string();
        let eng_str = eng.to_string();
        let handle = std::thread::spawn(move || {
            let bench = benchmark_single_engine(&eng_str, &t, &tg);
            res_clone.lock().unwrap().push(bench);
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    let mut list = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    list.sort_by(|a, b| {
        match (a.success, b.success) {
            (true, true) => a.latency_ms.cmp(&b.latency_ms),
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => a.id.cmp(&b.id),
        }
    });
    list
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engines_individually() {
        let client = reqwest::blocking::Client::new();
        
        let pa = translate_google_pa(&client, "Hello world", "vi");
        assert!(pa.is_ok(), "Google PA: {:?}", pa.err());

        let ms = translate_microsoft(&client, "Hello world", "vi");
        assert!(ms.is_ok(), "Microsoft: {:?}", ms.err());

        let ya = translate_yandex(&client, "Hello world", "vi");
        assert!(ya.is_ok(), "Yandex: {:?}", ya.err());

        let pa_naver = translate_papago(&client, "Hello world", "vi");
        assert!(pa_naver.is_ok(), "Papago: {:?}", pa_naver.err());

        let tc = translate_translate_com(&client, "Hello world", "vi");
        assert!(tc.is_ok(), "Translate.com: {:?}", tc.err());

        let mm = translate_mymemory(&client, "Hello world", "vi");
        assert!(mm.is_ok(), "MyMemory: {:?}", mm.err());

        let mmt = translate_modernmt(&client, "Hello world", "vi");
        assert!(mmt.is_ok(), "ModernMT: {:?}", mmt.err());
    }

    #[test]
    fn test_fallback_flow() {
        let res = translate_with_fallback("Hello world", "vi", "google_pa");
        assert!(res.is_ok(), "Translation failed: {:?}", res.err());
        let val = res.unwrap();
        assert!(!val.text.is_empty());
    }

    #[test]
    fn test_invalid_primary_triggers_fallback() {
        let res = translate_with_fallback("Hello world", "vi", "non_existent_engine");
        assert!(res.is_ok());
        let val = res.unwrap();
        assert_eq!(val.fallback_from, Some("non_existent_engine".into()));
        assert!(!val.text.is_empty());
    }
}
