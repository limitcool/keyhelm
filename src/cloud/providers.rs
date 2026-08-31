//! 云厂商 verify / probe 实现
//!
//! 设计：`verify` 只做轻量鉴权确认（返回账号身份），`probe` 尝试列举资源。

use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::CloudKeys;

/// 从 keys 里取某个 key_name，缺失返回错误
fn need<'a>(keys: &'a CloudKeys, name: &str) -> Result<&'a str, String> {
    keys.keys
        .get(name)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("缺少密钥 {name}（请在 project {} 下添加）", keys.provider))
}

fn hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    type HmacSha1 = Hmac<Sha1>;
    let mut mac = HmacSha1::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha1_hex(data: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(data);
    hex(&h.finalize())
}

fn now_rfc1123() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let days = (secs / 86400) as i64;
    let (y, m, d) = civil_from_days(days);
    let rem = secs % 86400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let wd = weekday(days);
    const WD: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MON: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
        WD[wd],
        d,
        MON[(m - 1) as usize],
        y,
        hh,
        mm,
        ss
    )
}

fn weekday(days: i64) -> usize {
    // 1970-01-01 是周四(4)。days 取模 7。
    ((((days % 7 + 7) % 7) + 4) % 7) as usize
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // YYYY-MM-DDTHH:MM:SSZ — 用 UTC
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn urlencode(s: &str) -> String {
    // RPC 签名要求对 query 做规范化：percent-encode 但不编码 / ~
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn query_escape(s: &str) -> String {
    // 与 urlencode 相同（阿里云用 percentEncode，也含空格→%20 等）
    urlencode(s)
}

// ==================== 阿里云 ====================
// 签名：RPC 风格，HMAC-SHA1（老式）或 HMAC-SHA256（新式 v3）
// 这里用 STS 的 GetCallerIdentity（无需额外参数），走 v3 样式 HMAC-SHA256。

async fn aliyun_request(
    keys: &CloudKeys,
    params: &[(&str, &str)],
    host: &str,
    api_path: &str,
    method: &str,
) -> Result<Value, String> {
    let access_key_id = need(keys, "ALIYUN_ACCESS_KEY_ID")?;
    let access_key_secret = need(keys, "ALIYUN_ACCESS_KEY_SECRET")?;

    let mut all: Vec<(String, String)> = vec![
        ("AccessKeyId".into(), access_key_id.to_string()),
        ("Action".into(), params[0].1.to_string()),
        ("Format".into(), "JSON".into()),
        ("SignatureMethod".into(), "HMAC-SHA1".into()),
        ("SignatureNonce".into(), uuid::Uuid::new_v4().to_string()),
        ("SignatureVersion".into(), "1.0".into()),
        ("Timestamp".into(), now_iso8601()),
        ("Version".into(), params[1].1.to_string()),
    ];
    for &(k, v) in &params[2..] {
        all.push((k.to_string(), v.to_string()));
    }
    all.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical = all
        .iter()
        .map(|(k, v)| format!("{}={}", query_escape(k), query_escape(v)))
        .collect::<Vec<_>>()
        .join("&");

    let string_to_sign = format!(
        "{method}&{0}&{1}",
        query_escape(api_path),
        query_escape(&canonical)
    );
    let sign = {
        use base64::Engine;
        let mac = hmac_sha1(
            format!("{access_key_secret}&").as_bytes(),
            string_to_sign.as_bytes(),
        );
        base64::engine::general_purpose::STANDARD.encode(mac)
    };

    let url = format!(
        "https://{host}{api_path}?{canonical}&Signature={}",
        query_escape(&sign)
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    if !status.is_success() {
        return Err(format!(
            "阿里云 API 返回 {status}: {}",
            v.get("Message").and_then(|m| m.as_str()).unwrap_or(&text)
        ));
    }
    Ok(v)
}

/// 阿里云验证：STS GetCallerIdentity 返回账号身份 + RAM 用户名
pub async fn aliyun_verify(keys: &CloudKeys) -> Result<Value, String> {
    let v = aliyun_request(
        keys,
        &[("Action", "GetCallerIdentity"), ("Version", "2015-04-01")],
        "sts.aliyuncs.com",
        "/",
        "GET",
    )
    .await?;
    let arn = v["Arn"].as_str().unwrap_or("");
    // ARN 形如 acs:ram::<uid>:user/<name>，取最后的用户名
    let user = arn.rsplit(':').next().and_then(|s| s.split('/').last());
    Ok(json!({
        "provider": "aliyun",
        "valid": true,
        "account_id": v["AccountId"].as_str(),
        "arn": arn,
        "user": user,
        "principal_id": v["PrincipalId"].as_str(),
    }))
}

/// 从 RAM ARN 中取用户名（acs:ram::<uid>:user/<name>）
fn ram_user_from_arn(arn: &str) -> Option<String> {
    let seg = arn.rsplit(':').next()?; // user/<name>
    let name = seg.split('/').last()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// 阿里云 OSS GetService（列举 bucket，SignatureV1 + XML）
async fn aliyun_oss_buckets(
    access_key_id: &str,
    access_key_secret: &str,
) -> Result<(Vec<String>, String), String> {
    let date = now_rfc1123();
    // GetService 的 StringToSign：GET\n\n\n<Date>\n/  （无 Content-MD5/Type，CanonicalizedResource=/）
    let string_to_sign = format!("GET\n\n\n{date}\n/");
    let sign = {
        use base64::Engine;
        let mac = hmac_sha1(access_key_secret.as_bytes(), string_to_sign.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(mac)
    };

    let client = reqwest::Client::new();
    let resp = client
        .get("https://oss.aliyuncs.com/")
        .header("Date", &date)
        .header("Authorization", format!("OSS {access_key_id}:{sign}"))
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    if !status.is_success() {
        let code = extract_xml(&text, "Code").unwrap_or_else(|| text.clone());
        let msg = extract_xml(&text, "Message").unwrap_or_default();
        return Err(format!("OSS 返回 {status}: {code} {msg}"));
    }
    // 解析 ListAllMyBucketsResult 的 Bucket.Name 列表
    let buckets: Vec<String> = text
        .split("<Bucket>")
        .skip(1)
        .filter_map(|seg| extract_xml(seg, "Name"))
        .collect();
    let region = extract_xml(&text, "Region").unwrap_or_default();
    Ok((buckets, region))
}

/// 阿里云探测：身份 + RAM 权限 + OSS bucket
/// 逐项探测，单项失败收集到 errors，不整体中断，让用户看到能访问的部分。
pub async fn aliyun_probe(keys: &CloudKeys) -> Result<Value, String> {
    let access_key_id = need(keys, "ALIYUN_ACCESS_KEY_ID")?.to_string();
    let access_key_secret = need(keys, "ALIYUN_ACCESS_KEY_SECRET")?.to_string();

    let mut errors: Vec<String> = Vec::new();

    // 1. 身份（STS GetCallerIdentity → account_id + RAM 用户）
    let mut account_id: Option<String> = None;
    let mut user: Option<String> = None;
    match aliyun_request(
        keys,
        &[("Action", "GetCallerIdentity"), ("Version", "2015-04-01")],
        "sts.aliyuncs.com",
        "/",
        "GET",
    )
    .await
    {
        Ok(v) => {
            account_id = v["AccountId"].as_str().map(String::from);
            user = v["Arn"].as_str().and_then(ram_user_from_arn);
        }
        Err(e) => errors.push(format!("身份查询: {e}")),
    }

    // 2. RAM 权限：列出该用户的系统策略 + 内联策略
    let mut policies: Vec<Value> = Vec::new();
    if let Some(name) = &user {
        let n = name.as_str();
        match aliyun_request(
            keys,
            &[
                ("Action", "ListPoliciesForUser"),
                ("Version", "2015-05-01"),
                ("UserName", n),
                ("PageSize", "100"),
            ],
            "ram.aliyuncs.com",
            "/",
            "GET",
        )
        .await
        {
            Ok(v) => {
                for p in v["Policies"]["Policy"].as_array().into_iter().flatten() {
                    policies.push(json!({
                        "name": p["PolicyName"],
                        "type": p["PolicyType"],
                        "default_version": p["DefaultVersion"],
                    }));
                }
            }
            Err(e) => errors.push(format!("权限查询: {e}")),
        }
        // 阿里云 RAM 无单独的「内联策略」RPC 列举动作，ListPoliciesForUser
        // 已返回用户绑定的系统策略；内联策略需要通过 GetUser 的关联查询，
        // 这里不再发无效请求。
    }

    // 3. OSS bucket
    let mut buckets: Vec<String> = Vec::new();
    let mut region: Option<String> = None;
    match aliyun_oss_buckets(&access_key_id, &access_key_secret).await {
        Ok((b, r)) => {
            buckets = b;
            region = if r.is_empty() { None } else { Some(r) };
        }
        Err(e) => errors.push(e),
    }

    Ok(json!({
        "provider": "aliyun",
        "probe": "identity+permissions+oss",
        "account_id": account_id,
        "user": user,
        "policies": policies,
        "buckets": buckets,
        "region": region,
        "errors": errors,
    }))
}

/// 从 XML 片段里取某个标签的文本
fn extract_xml(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
}

// ==================== 腾讯云 ====================
// TC3-HMAC-SHA256 签名，POST JSON。用 sts:GetCallerIdentity 验证。

async fn tencent_request(
    keys: &CloudKeys,
    action: &str,
    version: &str,
    payload: Value,
) -> Result<Value, String> {
    let secret_id = need(keys, "TENCENT_SECRET_ID")?;
    let secret_key = need(keys, "TENCENT_SECRET_KEY")?;

    let service = "sts";
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ts_str = ts.to_string();
    let date = {
        let days = ts / 86400;
        let (y, m, d) = civil_from_days(days as i64);
        format!("{y:04}-{m:02}-{d:02}")
    };

    let body = payload.to_string();
    let hashed_payload = sha256_hex(body.as_bytes());

    let canonical_headers = format!("content-type:application/json; charset=utf-8\nhost:sts.tencentcloudapi.com\nx-tc-action:{}\n", action.to_lowercase());
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request =
        format!("POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}");

    let credential_scope = format!("{date}/{service}/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{ts_str}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );

    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256(&secret_date, service.as_bytes());
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
    // TC3 规范：Signature 是 hex 编码（不是 base64！）
    let signature = hex(&hmac_sha256(&secret_signing, string_to_sign.as_bytes()));

    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let client = reqwest::Client::new();
    let resp = client
        .post("https://sts.tencentcloudapi.com/")
        .header("Authorization", &authorization)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Host", "sts.tencentcloudapi.com")
        .header("X-TC-Action", action)
        .header("X-TC-Version", version)
        .header("X-TC-Timestamp", &ts_str)
        .header("X-TC-Region", "ap-guangzhou")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    // 腾讯云即使业务失败也返回 HTTP 200，错误在 Response.Error 里，必须检查
    if !status.is_success() || v["Response"]["Error"].get("Code").is_some() {
        let msg = v["Response"]["Error"]["Message"].as_str().unwrap_or(&text);
        return Err(format!("腾讯云 API 返回 {status}: {msg}"));
    }
    Ok(v)
}

/// 腾讯云验证：STS GetCallerIdentity
pub async fn tencent_verify(keys: &CloudKeys) -> Result<Value, String> {
    let v = tencent_request(keys, "GetCallerIdentity", "2018-08-13", json!({})).await?;
    let r = &v["Response"];
    Ok(json!({
        "provider": "tencent",
        "valid": true,
        "account_id": r["AccountId"].as_str(),
        "arn": r["Arn"].as_str(),
        "user_id": r["UserId"].as_str(),
    }))
}

/// 腾讯云探测：列出 COS bucket（GetService，COS 签名域）
pub async fn tencent_probe(keys: &CloudKeys) -> Result<Value, String> {
    let secret_id = need(keys, "TENCENT_SECRET_ID")?.to_string();
    let secret_key = need(keys, "TENCENT_SECRET_KEY")?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let end = ts + 900; // 签名有效期（官方 SDK 默认 900s）
    let key_time = format!("{ts};{end}");
    // COS 规范（https://cloud.tencent.com/document/product/436/7778）：
    //   1. SignKey  = HMAC-SHA1(SecretKey, KeyTime)，hex 编码后作为后续 HMAC 密钥
    //   2. FormatString = Method\nPathname\nHttpParams\nHttpHeaders\n
    //   3. StringToSign  = sha1\nKeyTime\nSHA1(FormatString)hex\n
    //   4. Signature = HMAC-SHA1(SignKey, StringToSign)，hex 编码（不是 base64！）
    let sign_key = hex(&hmac_sha1(secret_key.as_bytes(), key_time.as_bytes()));
    let format_string = "get\n/\n\nhost=service.cos.myqcloud.com\n";
    let format_sha1 = sha1_hex(format_string.as_bytes());
    let string_to_sign = format!("sha1\n{key_time}\n{format_sha1}\n");
    let signature = hex(&hmac_sha1(sign_key.as_bytes(), string_to_sign.as_bytes()));
    let authorization = format!(
        "q-sign-algorithm=sha1&q-ak={secret_id}&q-sign-time={key_time}&q-key-time={key_time}&q-header-list=host&q-url-param-list=&q-signature={signature}"
    );

    let client = reqwest::Client::new();
    let resp = client
        .get("https://service.cos.myqcloud.com/")
        .header("Host", "service.cos.myqcloud.com")
        .header("Authorization", &authorization)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    if !status.is_success() {
        let code = extract_xml(&text, "Code").unwrap_or_else(|| text.clone());
        let msg = extract_xml(&text, "Message").unwrap_or_default();
        return Err(format!("腾讯云 COS 返回 {status}: {code} {msg}"));
    }
    // 解析 ListAllMyBucketsResult 的 Bucket.Name（格式：<appid>-<bucket>）
    let buckets: Vec<String> = text
        .split("<Bucket>")
        .skip(1)
        .filter_map(|seg| extract_xml(seg, "Name"))
        .collect();
    Ok(json!({
        "provider": "tencent",
        "probe": "cos-buckets",
        "buckets": buckets,
    }))
}

// ==================== Cloudflare ====================
// 简单 Bearer token。GET /user/tokens/verify 验证，GET /accounts 与 /zones 探测。

async fn cf_get(keys: &CloudKeys, path: &str) -> Result<(reqwest::StatusCode, Value), String> {
    let token = need(keys, "CLOUDFLARE_API_TOKEN")?;
    let client = reqwest::Client::new();
    let url = format!("https://api.cloudflare.com/client/v4{path}");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    Ok((status, v))
}

/// Cloudflare 验证：/user/tokens/verify
pub async fn cloudflare_verify(keys: &CloudKeys) -> Result<Value, String> {
    let (status, v) = cf_get(keys, "/user/tokens/verify").await?;
    if !status.is_success() || v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) != true {
        let errs = v["errors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| e["message"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        return Err(format!("Cloudflare token 无效: {errs}"));
    }
    let r = &v["result"];
    Ok(json!({
        "provider": "cloudflare",
        "valid": true,
        "token_id": r["id"].as_str(),
        "status": r["status"].as_str(),
        "issued_at": r["issued_on"].as_str(),
        "expires_at": r["expires_on"].as_str().filter(|s| !s.is_empty()),
    }))
}

/// Cloudflare 探测：列出账号与 zones
pub async fn cloudflare_probe(keys: &CloudKeys) -> Result<Value, String> {
    let (_, accounts) = cf_get(keys, "/accounts?per_page=50").await?;
    let account_names: Vec<Value> = accounts["result"]
        .as_array()
        .map(|a| a.iter().map(|x| x["name"].clone()).collect())
        .unwrap_or_default();
    let (_, zones) = cf_get(keys, "/zones?per_page=50").await?;
    let zone_names: Vec<Value> = zones["result"]
        .as_array()
        .map(|a| a.iter().map(|x| x["name"].clone()).collect())
        .unwrap_or_default();
    Ok(json!({
        "provider": "cloudflare",
        "accounts": account_names,
        "zones": zone_names,
        "note": "能访问的账号/域名列表",
    }))
}

// ==================== Google Cloud ====================
// Service Account JSON（GOOGLE_SERVICE_ACCOUNT_KEY）→ JWT 签名 → OAuth2 token → 列 projects。

fn google_base64url(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

fn rsa_sha256_sign(private_key_pem: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::{Pkcs1v15Sign, RsaPrivateKey};
    use sha2::{Digest, Sha256};
    let key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)
        .or_else(|_| rsa::pkcs1::DecodeRsaPrivateKey::from_pkcs1_pem(private_key_pem))
        .map_err(|e| format!("解析 RSA 私钥失败: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    key.sign(Pkcs1v15Sign::new::<Sha256>(), &digest)
        .map_err(|e| format!("RSA 签名失败: {e}"))
}

async fn google_token(keys: &CloudKeys) -> Result<(String, String), String> {
    let sa: Value = serde_json::from_str(need(keys, "GOOGLE_SERVICE_ACCOUNT_KEY")?)
        .map_err(|e| format!("GOOGLE_SERVICE_ACCOUNT_KEY 不是合法 JSON: {e}"))?;
    let client_email = sa["client_email"].as_str().ok_or("缺少 client_email")?;
    let private_key = sa["private_key"].as_str().ok_or("缺少 private_key")?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let header = google_base64url(b"{\"alg\":\"RS256\",\"typ\":\"JWT\"}");
    let claims = format!(
        "{{\"iss\":\"{client_email}\",\"scope\":\"https://www.googleapis.com/auth/cloud-platform\",\"aud\":\"https://oauth2.googleapis.com/token\",\"iat\":{now},\"exp\":{}}}",
        now + 3600
    );
    let claims_b64 = google_base64url(claims.as_bytes());
    let signing_input = format!("{header}.{claims_b64}");
    let sig = rsa_sha256_sign(private_key, signing_input.as_bytes())?;
    let jwt = format!("{signing_input}.{}", google_base64url(&sig));

    let client = reqwest::Client::new();
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer&assertion={jwt}"
        ))
        .send()
        .await
        .map_err(|e| format!("OAuth 请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    if !status.is_success() {
        return Err(format!(
            "Google OAuth 失败: {}",
            v["error_description"].as_str().unwrap_or(&text)
        ));
    }
    let access_token = v["access_token"]
        .as_str()
        .ok_or("缺少 access_token")?
        .to_string();
    Ok((access_token, client_email.to_string()))
}

/// Google 验证：OAuth 换 token 后列出 projects
pub async fn google_verify(keys: &CloudKeys) -> Result<Value, String> {
    let (token, email) = google_token(keys).await?;
    let client = reqwest::Client::new();
    let resp = client
        .get("https://cloudresourcemanager.googleapis.com/v1/projects")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    if !status.is_success() {
        return Err(format!(
            "Google Cloud API 失败: {status}: {}",
            v["error"]["message"].as_str().unwrap_or(&text)
        ));
    }
    Ok(json!({
        "provider": "google-cloud",
        "valid": true,
        "service_account": email,
        "projects": v["projects"].as_array().map(|a| a.iter().map(|p| json!({
            "id": p["projectId"],
            "name": p["name"],
            "number": p["projectNumber"],
        })).collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

/// Google 探测：列出 projects（与 verify 相同，但更聚焦资源）
pub async fn google_probe(keys: &CloudKeys) -> Result<Value, String> {
    let v = google_verify(keys).await?;
    Ok(json!({
        "provider": "google-cloud",
        "probe": "projects",
        "projects": v["projects"],
    }))
}
