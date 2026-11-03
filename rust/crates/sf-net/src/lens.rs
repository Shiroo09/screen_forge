//! Google Lens request builders. Extracted from `ScreenForge/Translate/GoogleLensClient.cs`.
//!
//! The field layout is Google's internal Lens protobuf; it is reproduced exactly.
//! `uuid` is passed in rather than read from RNG so both builders are deterministic
//! and testable.

use crate::proto::ProtoWriter;

const PLATFORM_WEB: i32 = 3;
const SURFACE_CHROMIUM: i32 = 4;
const FILTER_TRANSLATE: i32 = 2;

fn write_context(ctx: &mut ProtoWriter, uuid: u64, with_filters: bool, target: &str, source: Option<&str>) {
    ctx.write_message(3, |rid| {
        rid.write_uint64(1, uuid);
        rid.write_int32(2, 1);
        rid.write_int32(3, 1);
    });
    ctx.write_message(4, |client| {
        client.write_enum(1, PLATFORM_WEB);
        client.write_enum(2, SURFACE_CHROMIUM);
        client.write_message(4, |locale| {
            locale.write_string(2, "US");
            locale.write_string(3, "America/New_York");
        });
        if with_filters {
            client.write_message(17, |filters| {
                filters.write_message(1, |filter| {
                    filter.write_enum(1, FILTER_TRANSLATE);
                    filter.write_message(3, |tr| {
                        tr.write_string(1, target);
                        if let Some(src) = source {
                            if !src.trim().is_empty() {
                                tr.write_string(2, src);
                            }
                        }
                    });
                });
            });
        }
    });
}

fn write_image(objects: &mut ProtoWriter, image_bytes: &[u8], width: i32, height: i32) {
    objects.write_message(3, |image| {
        image.write_message(1, |payload| payload.write_bytes(1, image_bytes));
        image.write_message(3, |meta| {
            meta.write_int32(1, width);
            meta.write_int32(2, height);
        });
    });
}

/// Translation request: same envelope as OCR, plus a translate filter with the
/// target (and optional source) language.
pub fn build_request(
    image_bytes: &[u8],
    width: i32,
    height: i32,
    target_lang: &str,
    source_lang: Option<&str>,
    uuid: u64,
) -> Vec<u8> {
    let mut root = ProtoWriter::new();
    root.write_message(1, |objects| {
        objects.write_message(1, |ctx| write_context(ctx, uuid, true, target_lang, source_lang));
        write_image(objects, image_bytes, width, height);
    });
    root.into_bytes()
}

/// OCR request: the same envelope without the translate filter.
pub fn build_ocr_request(image_bytes: &[u8], width: i32, height: i32, uuid: u64) -> Vec<u8> {
    let mut root = ProtoWriter::new();
    root.write_message(1, |objects| {
        objects.write_message(1, |ctx| write_context(ctx, uuid, false, "", None));
        write_image(objects, image_bytes, width, height);
    });
    root.into_bytes()
}
