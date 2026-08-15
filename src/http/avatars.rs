use sha2::{Digest, Sha256};
use worker::*;

use crate::auth::google::extract_user;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::profiles::MyProfile;
use crate::http::response::{json_success, with_cors};
use crate::storage::d1::ProfileRepository;

pub const MAX_AVATAR_SIZE: usize = 2 * 1024 * 1024; // 2MB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFormat {
    pub ext: &'static str,
    pub mime: &'static str,
}

/// Detect image format by inspecting magic bytes.
/// Supported formats: JPEG, PNG, WebP.
pub fn detect_image_format(bytes: &[u8]) -> Result<ImageFormat, AppError> {
    if bytes.is_empty() {
        return Err(AppError::BadRequest("Image payload is empty.".to_string()));
    }
    if bytes.len() > MAX_AVATAR_SIZE {
        return Err(AppError::BadRequest(
            "Image size exceeds maximum allowed limit of 2MB.".to_string(),
        ));
    }

    if bytes.len() >= 3 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(ImageFormat {
            ext: "jpg",
            mime: "image/jpeg",
        });
    }

    if bytes.len() >= 8 && bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Ok(ImageFormat {
            ext: "png",
            mime: "image/png",
        });
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok(ImageFormat {
            ext: "webp",
            mime: "image/webp",
        });
    }

    Err(AppError::BadRequest(
        "Unsupported image format. Allowed formats: JPEG, PNG, WebP".to_string(),
    ))
}

/// Generate R2 key for avatar: avatars/{sha256(email)[..16]}_{timestamp}.{ext}
pub fn generate_avatar_key(email: &str, ext: &str, timestamp: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(email.trim().to_lowercase().as_bytes());
    let hash = hasher.finalize();
    let hash_hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    let hash_prefix = &hash_hex[..16];
    format!("avatars/{hash_prefix}_{timestamp}.{ext}")
}

/// POST /api/profiles/me/avatar — Upload custom profile picture to Cloudflare R2.
pub async fn handle_profiles_me_avatar_post(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let user = extract_user(&req, &config, Some(&db)).await?;
    let email = user.normalized_email();

    let bytes = req
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read request body: {e}")))?;

    let format = detect_image_format(&bytes)?;

    let timestamp = (js_sys::Date::now() / 1000.0) as u64;
    let key = generate_avatar_key(&email, format.ext, timestamp);

    let bucket = ctx.bucket("AVATAR_BUCKET")?;
    bucket
        .put(&key, bytes)
        .http_metadata(worker::HttpMetadata {
            content_type: Some(format.mime.to_string()),
            ..Default::default()
        })
        .execute()
        .await?;

    let public_url = format!("{}/{}", config.avatar_public_base_url.trim_end_matches('/'), key);

    ProfileRepository::update_picture(&db, &email, Some(&public_url))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update profile picture in database: {e}")))?;

    let my = match ProfileRepository::get_profile_by_email(&db, &email).await {
        Ok(Some(row)) => MyProfile::from_row(email, &row),
        Ok(None) => MyProfile::empty(email, Some(public_url)),
        Err(e) => return Err(AppError::Internal(format!("Failed to load profile: {e}")).into()),
    };

    let resp = json_success(&my)?;
    with_cors(resp, &config.allowed_origins)
}

/// DELETE /api/profiles/me/avatar — Reset custom profile picture back to provider default.
pub async fn handle_profiles_me_avatar_delete(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let user = extract_user(&req, &config, Some(&db)).await?;
    let email = user.normalized_email();

    ProfileRepository::update_picture(&db, &email, user.picture.as_deref())
        .await
        .map_err(|e| AppError::Internal(format!("Failed to reset profile picture in database: {e}")))?;

    let my = match ProfileRepository::get_profile_by_email(&db, &email).await {
        Ok(Some(row)) => MyProfile::from_row(email, &row),
        Ok(None) => MyProfile::empty(email, user.picture.clone()),
        Err(e) => return Err(AppError::Internal(format!("Failed to load profile: {e}")).into()),
    };

    let resp = json_success(&my)?;
    with_cors(resp, &config.allowed_origins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_jpeg() {
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        let detected = detect_image_format(&jpeg_bytes).unwrap();
        assert_eq!(
            detected,
            ImageFormat {
                ext: "jpg",
                mime: "image/jpeg"
            }
        );
    }

    #[test]
    fn test_detect_png() {
        let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        let detected = detect_image_format(&png_bytes).unwrap();
        assert_eq!(
            detected,
            ImageFormat {
                ext: "png",
                mime: "image/png"
            }
        );
    }

    #[test]
    fn test_detect_webp() {
        let mut webp_bytes = vec![0u8; 16];
        webp_bytes[0..4].copy_from_slice(b"RIFF");
        webp_bytes[4..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        webp_bytes[8..12].copy_from_slice(b"WEBP");
        let detected = detect_image_format(&webp_bytes).unwrap();
        assert_eq!(
            detected,
            ImageFormat {
                ext: "webp",
                mime: "image/webp"
            }
        );
    }

    #[test]
    fn test_reject_empty_payload() {
        let err = detect_image_format(&[]).unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("empty")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[test]
    fn test_reject_unsupported_format() {
        let gif_bytes = b"GIF89a...";
        let err = detect_image_format(gif_bytes).unwrap_err();
        match err {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("Unsupported image format"));
            }
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[test]
    fn test_reject_oversized_payload() {
        let oversized = vec![0xFF, 0xD8, 0xFF];
        let mut large_vec = vec![0u8; MAX_AVATAR_SIZE + 1];
        large_vec[0..3].copy_from_slice(&oversized);
        let err = detect_image_format(&large_vec).unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("2MB")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[test]
    fn test_generate_avatar_key() {
        let email = "test.user@example.com";
        let key = generate_avatar_key(email, "png", 1700000000);
        let mut hasher = Sha256::new();
        hasher.update(b"test.user@example.com");
        let expected_hash_hex: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        let expected_prefix = &expected_hash_hex[..16];
        assert_eq!(key, format!("avatars/{expected_prefix}_1700000000.png"));
    }
}
