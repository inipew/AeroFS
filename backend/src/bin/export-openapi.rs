use backend::api::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let doc = ApiDoc::openapi();
    let json =
        serde_json::to_string_pretty(&doc).expect("Failed to serialize OpenAPI spec to JSON");
    println!("{}", json);
}
