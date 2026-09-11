//! The OpenAPI specification, generated from the DTOs themselves.
//!
//! Schemas come from `utoipa::ToSchema` on the types in [`crate::dto`], so changing a DTO
//! field changes the spec — there is no second hand-written description to drift from the
//! real contract.
//!
//! Paths are described by hand, but **the path strings come from [`crate::route::pattern`]**,
//! exactly the strings the router uses. The spec therefore cannot point at an endpoint that
//! does not exist.

use utoipa::openapi::path::{HttpMethod, OperationBuilder, ParameterBuilder, ParameterIn};
use utoipa::openapi::{
    ComponentsBuilder, ContentBuilder, InfoBuilder, OpenApiBuilder, PathItem, PathsBuilder,
    Required, ResponseBuilder, ResponsesBuilder, ServerBuilder,
};
use utoipa::{PartialSchema, ToSchema};

use crate::dto;
use crate::route::pattern;

const JSON: &str = "application/json";

/// The specification as a JSON string.
pub fn document() -> String {
    build().to_pretty_json().unwrap_or_else(|e| {
        // No panic on the serving path: a broken spec is not worth taking the server down.
        format!("{{\"error\":\"không dựng được đặc tả: {e}\"}}")
    })
}

fn build() -> utoipa::openapi::OpenApi {
    let info = InfoBuilder::new()
        .title("Đại Nam Quấc Âm Tự Vị — API")
        .version(dto::API_VERSION)
        .description(Some(
            "API mở cho bản điện tử 2026 của Đại Nam Quấc Âm Tự Vị (Huình-Tịnh Paulus Của, \
             1895–1896).\n\n\
             Hai quy ước cần biết trước khi dùng:\n\n\
             1. Mỗi mục con có `form` (**nguyên văn bản in**, còn dấu thế chỗ như `― gươm`) và \
             `form_expanded` (**suy diễn**, `Lõm gươm`). Đừng dùng lẫn: một cái là lời sách, \
             một cái là thứ chương trình suy ra theo quy tắc ở trang DẤU RIÊNG.\n\n\
             2. Dấu thanh có nghĩa phân biệt. `Ả`, `Á`, `À` là ba chữ đầu khác nhau, và API \
             không bao giờ tự bỏ dấu chữ bạn gõ của bạn.",
        ))
        .build();

    let components = ComponentsBuilder::new()
        .schema_from::<dto::EntryDto>()
        .schema_from::<dto::EntryDetailDto>()
        .schema_from::<dto::SubEntryDto>()
        .schema_from::<dto::GlyphDto>()
        .schema_from::<dto::ScoredEntryDto>()
        .schema_from::<dto::SearchResponseDto>()
        .schema_from::<dto::SuggestionDto>()
        .schema_from::<dto::PageDto>()
        .schema_from::<dto::StatsDto>()
        .build();

    let paths = PathsBuilder::new()
        .path(
            pattern::API_SEARCH,
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .summary(Some("Tra tìm"))
                    .description(Some(
                        "Trả kết quả kèm **lý do khớp** (`tier`), và gợi ý chính tả 1895 ↔ nay. \
                         Gợi ý chỉ là gợi ý: chữ bạn gõ của bạn không bao giờ bị viết lại.",
                    ))
                    .parameter(
                        ParameterBuilder::new()
                            .name("q")
                            .parameter_in(ParameterIn::Query)
                            .required(Required::True)
                            .description(Some("Truy vấn: âm Quốc ngữ, chữ Hán-Nôm, hoặc cụm từ."))
                            .schema(Some(String::schema()))
                            .build(),
                    )
                    .parameter(
                        ParameterBuilder::new()
                            .name("che_do")
                            .parameter_in(ParameterIn::Query)
                            .description(Some(
                                "auto · han-nom · quoc-ngu · toan-van. Không nói gì thì là `auto`.",
                            ))
                            .schema(Some(String::schema()))
                            .build(),
                    )
                    .parameter(
                        ParameterBuilder::new()
                            .name("trang")
                            .parameter_in(ParameterIn::Query)
                            .description(Some("Số trang kết quả, bắt đầu từ 1."))
                            .schema(Some(u64::schema()))
                            .build(),
                    )
                    .responses(json_response::<dto::SearchResponseDto>("Kết quả tra tìm"))
                    .build(),
            ),
        )
        .path(
            pattern::API_ENTRY,
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .summary(Some("Một chữ đầu, kèm mọi mục con"))
                    .parameter(slug_param())
                    .responses(json_response::<dto::EntryDetailDto>("Chữ đầu"))
                    .build(),
            ),
        )
        .path(
            pattern::API_GLYPH,
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .summary(Some("Mọi chữ đầu dùng một tự dạng"))
                    .description(Some(
                        "Một tự dạng Hán-Nôm thường có nhiều âm đọc; đây là cấu trúc của bảng \
                         Mục Từ đi kèm bản điện tử.",
                    ))
                    .parameter(
                        ParameterBuilder::new()
                            .name("glyph")
                            .parameter_in(ParameterIn::Path)
                            .required(Required::True)
                            .description(Some("Đúng một con chữ Hán-Nôm, đã mã hoá URL."))
                            .schema(Some(String::schema()))
                            .build(),
                    )
                    .responses(json_response::<dto::EntryDto>("Danh sách chữ đầu"))
                    .build(),
            ),
        )
        .path(
            pattern::API_PAGE,
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .summary(Some("Một trang in và các chữ đầu của nó"))
                    .description(Some(
                        "`pdf_page` là số trang của bản điện tử; `printed_page` là số in trên \
                         giấy. Quan hệ giữa hai hệ **không đồng nhất**: ba trang không có số in.",
                    ))
                    .parameter(
                        ParameterBuilder::new()
                            .name("page")
                            .parameter_in(ParameterIn::Path)
                            .required(Required::True)
                            .description(Some(
                                "Số in trên giấy, 3–1037. Ba trang không có số in gọi bằng                                  `pdf-1`, `pdf-2`, `pdf-10`.",
                            ))
                            .schema(Some(String::schema()))
                            .build(),
                    )
                    .responses(json_response::<dto::PageDto>("Trang in"))
                    .build(),
            ),
        )
        .path(
            pattern::API_STATS,
            PathItem::new(
                HttpMethod::Get,
                OperationBuilder::new()
                    .summary(Some("Số liệu phẩm chất dữ liệu"))
                    .description(Some(
                        "Bày ra chỗ dữ liệu còn yếu. Với một cuốn tự vị, nói rõ sai sót mới \
                         là chỗ đáng tin.",
                    ))
                    .responses(json_response::<dto::StatsDto>("Số liệu"))
                    .build(),
            ),
        )
        .build();

    OpenApiBuilder::new()
        .info(info)
        .servers(Some(vec![ServerBuilder::new().url("/").build()]))
        .components(Some(components))
        .paths(paths)
        .build()
}

fn slug_param() -> utoipa::openapi::path::Parameter {
    ParameterBuilder::new()
        .name("slug")
        .parameter_in(ParameterIn::Path)
        .required(Required::True)
        .description(Some(
            "Định danh trên URL, sinh từ âm đọc đã bỏ dấu; mục trùng âm mang hậu tố `-2`, `-3`…",
        ))
        .schema(Some(String::schema()))
        .build()
}

fn json_response<T: ToSchema + PartialSchema>(description: &str) -> utoipa::openapi::Responses {
    ResponsesBuilder::new()
        .response(
            "200",
            ResponseBuilder::new()
                .description(description)
                .content(
                    JSON,
                    ContentBuilder::new().schema(Some(T::schema())).build(),
                )
                .build(),
        )
        .response(
            "404",
            ResponseBuilder::new().description("Không tìm thấy").build(),
        )
        .build()
}
