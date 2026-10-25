/// Protobuf Varint Encoding
pub fn encode_varint(mut value: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    while value >= 0x80 {
        buf.push((value & 0x7F | 0x80) as u8);
        value >>= 7;
    }
    buf.push(value as u8);
    buf
}

/// Read Protobuf Varint
pub fn read_varint(data: &[u8], offset: usize) -> Result<(u64, usize), String> {
    let mut result = 0u64;
    let mut shift = 0;
    let mut pos = offset;

    loop {
        if pos >= data.len() {
            return Err("incomplete_data".to_string());
        }
        let byte = data[pos];
        if shift >= 64 || (shift == 63 && byte > 1) {
            return Err("varint_too_long".to_string());
        }
        result |= ((byte & 0x7F) as u64) << shift;
        pos += 1;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    Ok((result, pos))
}

/// Skip Protobuf Field
pub fn skip_field(data: &[u8], offset: usize, wire_type: u8) -> Result<usize, String> {
    let bounded_end = |end: usize| -> Result<usize, String> {
        if end > data.len() {
            Err("incomplete_data".to_string())
        } else {
            Ok(end)
        }
    };
    match wire_type {
        0 => {
            // Varint
            let (_, new_offset) = read_varint(data, offset)?;
            Ok(new_offset)
        }
        1 => {
            // 64-bit
            bounded_end(offset.checked_add(8).ok_or("incomplete_data".to_string())?)
        }
        2 => {
            // Length-delimited
            let (length, content_offset) = read_varint(data, offset)?;
            let end = content_offset
                .checked_add(usize::try_from(length).map_err(|_| "incomplete_data".to_string())?)
                .ok_or("incomplete_data".to_string())?;
            bounded_end(end)
        }
        5 => {
            // 32-bit
            bounded_end(offset.checked_add(4).ok_or("incomplete_data".to_string())?)
        }
        _ => Err(format!("unknown_wire_type: {}", wire_type)),
    }
}

/// Find specified Protobuf field content (Length-Delimited only)
pub fn find_field(data: &[u8], target_field: u32) -> Result<Option<Vec<u8>>, String> {
    let mut offset = 0;

    while offset < data.len() {
        let (tag, new_offset) = read_varint(data, offset)?;

        let wire_type = (tag & 7) as u8;
        let field_num = (tag >> 3) as u32;

        if field_num == target_field && wire_type == 2 {
            let (length, content_offset) = read_varint(data, new_offset)?;
            let end = content_offset
                .checked_add(usize::try_from(length).map_err(|_| "incomplete_data".to_string())?)
                .ok_or("incomplete_data".to_string())?;
            if end > data.len() {
                return Err("incomplete_data".to_string());
            }
            return Ok(Some(data[content_offset..end].to_vec()));
        }

        // Skip field
        offset = skip_field(data, new_offset, wire_type)?;
    }

    Ok(None)
}

/// 编码长度分隔字段 (wire_type = 2)
pub fn encode_len_delim_field(field_num: u32, data: &[u8]) -> Vec<u8> {
    let tag = (field_num << 3) | 2;
    let mut f = encode_varint(tag as u64);
    f.extend(encode_varint(data.len() as u64));
    f.extend_from_slice(data);
    f
}

/// 编码字符串字段 (wire_type = 2)
pub fn encode_string_field(field_num: u32, value: &str) -> Vec<u8> {
    encode_len_delim_field(field_num, value.as_bytes())
}

/// 编码 varint 字段 (wire_type = 0)
pub fn encode_varint_field(field_num: u32, value: u64) -> Vec<u8> {
    let tag = (field_num << 3) | 0;
    let mut f = encode_varint(tag as u64);
    f.extend(encode_varint(value));
    f
}

/// 创建 OAuthTokenInfo 消息（不包含 Field 6 包装，用于新格式）
pub fn create_oauth_info(
    access_token: &str,
    refresh_token: &str,
    expiry: i64,
    mut is_gcp_tos: bool,
    id_token: Option<&str>,
    email: Option<&str>,
) -> Vec<u8> {
    // 智能纠正 is_gcp_tos (兼容性核心逻辑)
    // 逻辑：如果确定是个人账号（通过邮件后缀），或者被明确要求修正，则强制关闭 Field 6
    if let Some(email_str) = email {
        let is_personal = email_str.to_lowercase().ends_with("@gmail.com")
            || email_str.to_lowercase().ends_with("@outlook.com")
            || email_str.to_lowercase().ends_with("@hotmail.com")
            || email_str.to_lowercase().ends_with("@qq.com")
            || email_str.to_lowercase().ends_with("@163.com");

        if is_personal && is_gcp_tos {
            crate::modules::logger::log_info(&format!(
                "[Protobuf] 自动纠正个人账号 ({}) 的 GCP 标志位以确保 IDE 刷新兼容性。",
                email_str
            ));
            is_gcp_tos = false;
        }
    }

    // Field 1: access_token
    let field1 = encode_string_field(1, access_token);

    // Field 2: token_type = "Bearer"
    let field2 = encode_string_field(2, "Bearer");

    // Field 3: refresh_token
    let field3 = encode_string_field(3, refresh_token);

    // Field 4: expiry (嵌套的 Timestamp 消息)
    // message Timestamp { int64 seconds = 1; int32 nanos = 2; }
    let seconds_tag = (1 << 3) | 0;
    let mut timestamp_msg = encode_varint(seconds_tag);
    timestamp_msg.extend(encode_varint(expiry as u64));

    // 添加 Field 2: nanos (0)
    let nanos_tag = (2 << 3) | 0;
    timestamp_msg.extend(encode_varint(nanos_tag));
    timestamp_msg.extend(encode_varint(0));

    let field4 = encode_len_delim_field(4, &timestamp_msg);

    // Field 5: id_token (如果存在)
    let field5 = id_token.map(|it| encode_string_field(5, it));

    // Field 6: is_gcp_tos
    let field6 = is_gcp_tos.then(|| encode_varint_field(6, 1));

    // 合并所有字段为 OAuthTokenInfo 消息
    let mut oauth_info = Vec::new();
    oauth_info.extend(field1);
    oauth_info.extend(field2);
    oauth_info.extend(field3);
    oauth_info.extend(field4);
    if let Some(field5) = field5 {
        oauth_info.extend(field5);
    }
    if let Some(field6) = field6 {
        oauth_info.extend(field6);
    }
    oauth_info
}

fn decode_legacy_base64_payload_if_needed(payload: Vec<u8>) -> Vec<u8> {
    use base64::{engine::general_purpose, Engine as _};

    let looks_like_legacy_base64 = payload.len() % 4 == 0
        && !payload.is_empty()
        && payload.iter().all(
            |byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'='),
        );

    if !looks_like_legacy_base64 {
        return payload;
    }

    let Ok(decoded) = general_purpose::STANDARD.decode(&payload) else {
        return payload;
    };

    if decoded.is_empty() {
        payload
    } else {
        decoded
    }
}

fn decode_topic_row_payload(topic_blob: &[u8]) -> Result<(String, Vec<u8>), String> {
    use base64::{engine::general_purpose, Engine as _};

    let data_entry = find_field(topic_blob, 1)?.ok_or("Topic data entry not found".to_string())?;
    let sentinel_key = String::from_utf8(
        find_field(&data_entry, 1)?.ok_or("Topic data entry key not found".to_string())?,
    )
    .map_err(|_| "Topic data entry key is not UTF-8".to_string())?;
    let row_blob = find_field(&data_entry, 2)?.ok_or("Topic row not found".to_string())?;
    let encoded_payload = String::from_utf8(
        find_field(&row_blob, 1)?.ok_or("Topic row value not found".to_string())?,
    )
    .map_err(|_| "Topic row value is not UTF-8".to_string())?;
    let payload = general_purpose::STANDARD
        .decode(encoded_payload)
        .map_err(|e| format!("Topic row payload base64 decoding failed: {}", e))?;

    Ok((sentinel_key, payload))
}

fn decode_legacy_unified_state_entry(outer_blob: &[u8]) -> Result<(String, Vec<u8>), String> {
    let inner_blob = find_field(outer_blob, 1)?.ok_or("Outer Field 1 not found".to_string())?;
    let sentinel_key = String::from_utf8(
        find_field(&inner_blob, 1)?.ok_or("Inner Field 1 not found".to_string())?,
    )
    .map_err(|_| "Sentinel key is not UTF-8".to_string())?;
    let payload = find_field(&inner_blob, 2)?.ok_or("Inner Field 2 not found".to_string())?;
    let payload = decode_legacy_base64_payload_if_needed(payload);

    Ok((sentinel_key, payload))
}

/// 创建统一状态同步条目：Topic(Field 1 data map) -> DataEntry(Field 1 key, Field 2 Row) -> Row(Field 1 base64 payload)
pub fn create_unified_state_entry(sentinel_key: &str, payload: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};

    let row = encode_string_field(1, &general_purpose::STANDARD.encode(payload));
    let data_entry = [
        encode_string_field(1, sentinel_key),
        encode_len_delim_field(2, &row),
    ]
    .concat();
    let topic = encode_len_delim_field(1, &data_entry);

    general_purpose::STANDARD.encode(topic)
}

/// 解码统一状态同步条目，返回 sentinel key 和原始 payload。
/// 优先支持官方 Topic/Row 格式，并兼容早期工具写入的错误嵌套格式。
pub fn decode_unified_state_entry(outer_b64: &str) -> Result<(String, Vec<u8>), String> {
    use base64::{engine::general_purpose, Engine as _};

    let outer_blob = general_purpose::STANDARD
        .decode(outer_b64)
        .map_err(|e| format!("Outer Base64 decoding failed: {}", e))?;

    decode_topic_row_payload(&outer_blob)
        .or_else(|_| decode_legacy_unified_state_entry(&outer_blob))
}

/// 查找指定 protobuf varint 字段
pub fn find_varint_field(data: &[u8], target_field: u32) -> Result<Option<u64>, String> {
    let mut offset = 0;

    while offset < data.len() {
        let (tag, new_offset) = read_varint(data, offset)?;
        let wire_type = (tag & 7) as u8;
        let field_num = (tag >> 3) as u32;

        if field_num == target_field && wire_type == 0 {
            let (value, _) = read_varint(data, new_offset)?;
            return Ok(Some(value));
        }

        offset = skip_field(data, new_offset, wire_type)?;
    }

    Ok(None)
}

/// 创建 unified-state stringValue payload
pub fn create_string_value_payload(value: &str) -> Vec<u8> {
    // Matches the upstream `fs` message: { value: { case: "stringValue", value } }
    encode_string_field(3, value)
}

/// 创建最小可用的 UserStatus payload。
///
/// Antigravity 的认证链路要求 `uss-userStatus` 里至少存在 sentinel key；
/// 账号展示和会话绑定依赖名字和邮箱，因此这里写入最小身份信息即可。
pub fn create_minimal_user_status_payload(email: &str) -> Vec<u8> {
    [encode_string_field(3, email), encode_string_field(7, email)].concat()
}

/// 创建 unified-state Topic.data entry。
pub fn create_unified_topic_entry(sentinel_key: &str, payload: &[u8]) -> Vec<u8> {
    use base64::{engine::general_purpose, Engine as _};
    let row = encode_string_field(1, &general_purpose::STANDARD.encode(payload));
    let entry = [
        encode_string_field(1, sentinel_key),
        encode_len_delim_field(2, &row),
    ]
    .concat();
    encode_len_delim_field(1, &entry)
}

/// 从 Topic.data 中移除指定 map entry，保留同 topic 下其他 sentinel row。
pub fn remove_unified_topic_entry(data: &[u8], target_key: &str) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let start_offset = offset;
        let (tag, new_offset) = read_varint(data, offset)?;
        let wire_type = (tag & 7) as u8;
        let field_num = (tag >> 3) as u32;
        let next_offset = skip_field(data, new_offset, wire_type)?;

        let should_remove = if field_num == 1 && wire_type == 2 {
            let (length, content_offset) = read_varint(data, new_offset)?;
            let end = content_offset.checked_add(usize::try_from(length).map_err(|_| "incomplete_data")?)
                .ok_or("incomplete_data")?;
            if end > data.len() {
                return Err("Topic.data entry 数据不完整".to_string());
            }
            let entry = &data[content_offset..end];
            unified_topic_entry_key(entry) == Some(target_key)
        } else {
            false
        };

        if !should_remove {
            result.extend_from_slice(&data[start_offset..next_offset]);
        }
        offset = next_offset;
    }

    Ok(result)
}

fn unified_topic_entry_key(data: &[u8]) -> Option<&str> {
    let mut offset = 0;
    while offset < data.len() {
        let (tag, new_offset) = read_varint(data, offset).ok()?;
        let wire_type = (tag & 7) as u8;
        let field_num = (tag >> 3) as u32;

        if field_num == 1 && wire_type == 2 {
            let (length, content_offset) = read_varint(data, new_offset).ok()?;
            let end = content_offset.checked_add(usize::try_from(length).ok()?)?;
            if end > data.len() {
                return None;
            }
            return std::str::from_utf8(&data[content_offset..end]).ok();
        }

        offset = skip_field(data, new_offset, wire_type).ok()?;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_length_delimited_field_fails_instead_of_panicking() {
        // Field 1 (wire type 2) declaring 15 content bytes with only 1 present.
        let truncated: &[u8] = &[0x0A, 0x0F, 0x01];
        assert!(find_field(truncated, 1).is_err());
        assert!(skip_field(truncated, 1, 2).is_err());

        // A length that overflows usize arithmetic must fail, not slice out of range.
        let oversized: &[u8] = &[0x0A, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F, 0x00];
        assert!(find_field(oversized, 1).is_err());
        let mut maximum_length = vec![0x0a];
        maximum_length.extend(encode_varint(u64::MAX));
        assert!(find_field(&maximum_length, 1).is_err());
        assert!(remove_unified_topic_entry(&maximum_length, "fixture").is_err());
        assert_eq!(unified_topic_entry_key(&maximum_length), None);
        let nested = encode_len_delim_field(1, &maximum_length);
        assert!(remove_unified_topic_entry(&nested, "fixture").is_ok());
    }

    #[test]
    fn oversized_varint_is_rejected() {
        // 11 continuation bytes exceed the 64-bit varint payload.
        let mut data = vec![0xFF; 11];
        data.push(0x01);
        assert!(read_varint(&data, 0).is_err());
        assert_eq!(read_varint(&[0xFF, 0x01], 0), Ok((255, 2)));
        for last in [2, 0x7f, 0x80, 0x81, 0xff] {
            let mut overflow = vec![0x80; 9];
            overflow.push(last);
            assert!(read_varint(&overflow, 0).is_err());
            assert!(find_field(&overflow, 1).is_err());
        }
        assert_eq!(read_varint(&encode_varint(u64::MAX), 0), Ok((u64::MAX, 10)));
    }

    #[test]
    fn skip_field_rejects_truncated_fixed_width_fields() {
        assert!(skip_field(&[0x00], 0, 1).is_err());
        assert!(skip_field(&[0x00, 0x00], 0, 5).is_err());
        assert!(skip_field(&[0x00, 0x00, 0x00, 0x00], 0, 5).is_ok());
    }

    #[test]
    fn valid_fields_still_parse() {
        // Field 1 string "hi", then field 2 varint 150.
        let data: &[u8] = &[0x0A, 0x02, b'h', b'i', 0x10, 0x96, 0x01];
        assert_eq!(find_field(data, 1), Ok(Some(b"hi".to_vec())));
        assert_eq!(find_varint_field(data, 2), Ok(Some(150)));
        assert_eq!(find_field(data, 3), Ok(None));
        assert_eq!(read_varint(&[0x96, 0x01], 0), Ok((150, 2)));
    }

    #[test]
    fn malformed_outer_entries_surface_as_decode_errors() {
        // decode_unified_state_entry must fail cleanly on corrupt base64 payloads
        // instead of panicking deep inside field parsing.
        use base64::{engine::general_purpose, Engine as _};
        let corrupt = general_purpose::STANDARD.encode([0x0A, 0x7F, 0x01]);
        assert!(decode_unified_state_entry(&corrupt).is_err());
        let valid = create_unified_state_entry("sentinel", &[0x01, 0x02]);
        assert!(decode_unified_state_entry(&valid).is_ok());
    }
}
