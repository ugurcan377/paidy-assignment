use axum::{
    extract::{State, Path},
    http::StatusCode,
    routing::{get, post, delete},
    response::IntoResponse,
    Json,
    Router,
    // debug_handler,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{FromRow, Transaction, Postgres};
use rand::Rng;
use tokio::time::Duration;
use std::env;

#[derive(Deserialize)]
struct OrderPayload{
    #[serde(default = "OrderPayload::default_name")]
    item_name: String,
    #[serde(default = "OrderPayload::default_duration")]
    duration: i32
}

impl OrderPayload {
    fn default_duration() -> i32 {
        rand::thread_rng().gen_range(5..15)
    }
    fn default_name() -> String {
        "".to_string()
    }
}

#[derive(Deserialize)]
struct OrderRequest {
    orders: Vec<OrderPayload>
}

#[derive(Serialize, FromRow)]
struct Order {
    id: i32,
    table_no: i32,
    item_name: String,
    duration: i32
}

#[derive(Serialize)]
struct Table {
    orders: Vec<Order>
}

#[tokio::main]
async fn main() {
    let db_url = env::var("DATABASE_URL").expect("Please set $DATABASE_URL");

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&db_url)
        .await
        .expect("can't connect to database");

    sqlx::migrate!().run(&pool).await.unwrap();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app(pool).await).await.unwrap();
}

async fn app(pool: PgPool) -> Router {

    Router::new()
    .route("/", get(|| async { "Paidy Restaurant API" }))
    .route("/tables", get(list_tables))
    .route("/tables/:id", get(get_table).post(post_table).delete(delete_table))
    .route("/orders/:id", get(get_order).delete(delete_order))
    .with_state(pool)
}

async fn insert_into_db(tx: &mut Transaction<'_, Postgres>, id: i32, item_name: String, duration: i32) {
    sqlx::query("insert into orders (table_no, item_name, duration) values ($1, $2, $3)")
        .bind(id)
        .bind(item_name)
        .bind(duration)
        .execute(&mut **tx)
        .await.unwrap();
}

async fn select_table(pool: &PgPool, id: i32) -> Vec<Order>{
    sqlx::query_as::<_, Order>(
        "SELECT id, table_no, item_name, duration FROM orders WHERE table_no = $1 AND deleted_at IS NULL")
       .bind(id)
       .fetch_all(pool)
       .await.unwrap()
}

async fn list_tables(State(pool): State<PgPool>,) -> Json<Table> {
    let orders: Vec<Order> = sqlx::query_as::<_, Order>(
        "SELECT id, table_no, item_name, duration FROM orders WHERE deleted_at IS NULL")
       .fetch_all(&pool)
       .await.unwrap();

    Json(Table{
        orders,
    })
}

async fn get_table(Path(id): Path<i32>, State(pool): State<PgPool>,) -> Json<Table> {
    let orders: Vec<Order> = select_table(&pool, id).await;

    Json(Table{
        orders
    })
}

async fn post_table(Path(id): Path<i32>, State(pool): State<PgPool>, Json(payload): Json<OrderRequest>) -> Json<Table>{
    let mut tx = pool.begin().await.unwrap();
    for order in payload.orders {
        if !order.item_name.is_empty() {
            insert_into_db(&mut tx, id, order.item_name, order.duration).await;
        }
    }
    tx.commit().await.unwrap();
    let orders: Vec<Order> = select_table(&pool, id).await;

    Json(Table{
        orders
    })
}

async fn delete_table(Path(id): Path<i32>, State(pool): State<PgPool>,) -> impl IntoResponse {
    let result = sqlx::query("UPDATE orders SET deleted_at = CURRENT_TIMESTAMP WHERE table_no = $1")
    .bind(id)
    .execute(&pool)
    .await.unwrap();

    if result.rows_affected() > 0 {
        return StatusCode::NO_CONTENT
    }
    return StatusCode::NOT_FOUND
}


async fn get_order(Path(id): Path<i32>, State(pool): State<PgPool>,) -> impl IntoResponse{
   let result = sqlx::query_as::<_, Order>(
    "SELECT id, table_no, item_name, duration FROM orders WHERE id = $1 AND deleted_at IS NULL")
   .bind(id)
   .fetch_one(&pool)
   .await;

   match result {
    Ok(order) => return (StatusCode::OK, Json(order)).into_response(),
    Err(_) => return StatusCode::NOT_FOUND.into_response()
   }
}

async fn delete_order(Path(id): Path<i32>, State(pool): State<PgPool>,) -> impl IntoResponse {
    let result = sqlx::query("UPDATE orders SET deleted_at = CURRENT_TIMESTAMP WHERE id = $1")
    .bind(id)
    .execute(&pool)
    .await.unwrap();

    if result.rows_affected() > 0 {
        return StatusCode::NO_CONTENT
    }
    return StatusCode::NOT_FOUND
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use http_body_util::BodyExt; // for `collect`
    use serde_json::{json, Value};
    use tower::{Service, ServiceExt}; // for `call`, `oneshot`, and `ready`

    #[sqlx::test]
    async fn test_root(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"Paidy Restaurant API");
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_tables(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": [{"item_name": "pasta", "duration": 14}]
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["orders"].as_array().unwrap().len(), 1);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_multiple_tables(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": [{"item_name": "pasta"}, {"item_name": "doria"}]
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["orders"].as_array().unwrap().len(), 2);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_multiple_tables_with_empty_items(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": [{"item_name": "pasta"}, {}]
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["orders"].as_array().unwrap().len(), 1);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_add_orders_to_table(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/14")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": [{"item_name": "pasta"}, {"item_name": "doria"}]
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["orders"].as_array().unwrap().len(), 4);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_empty_tables(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": []
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["orders"].as_array().unwrap().len(), 0);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_tables_wrong_type(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "orders": [{"item_name": 15}]
                        })).unwrap()
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_create_tables_empty_body(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/tables/5")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_list_tables(pool: PgPool) {
        let app = app(pool).await;
        let response = app
        .oneshot(Request::builder().uri("/tables").body(Body::empty()).unwrap())
        .await
        .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!({
            "orders": [
                {
                    "id": 1,
                    "table_no": 14,
                    "item_name": "karaage",
                    "duration": 10
                },
                {
                    "id": 2,
                    "table_no": 15,
                    "item_name": "yakisoba",
                    "duration": 7
                },
                {
                    "id": 3,
                    "table_no": 14,
                    "item_name": "takoyaki",
                    "duration": 6
                },
            ] 
        }));
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_get_tables(pool: PgPool) {
        let app = app(pool).await;
        let response = app
        .oneshot(Request::builder().uri("/tables/14").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!({
            "orders": [
                {
                    "id": 1,
                    "table_no": 14,
                    "item_name": "karaage",
                    "duration": 10
                },
                {
                    "id": 3,
                    "table_no": 14,
                    "item_name": "takoyaki",
                    "duration": 6
                },
            ] 
        }));
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_get_order(pool: PgPool) {
        let app = app(pool).await;
        let response = app
        .oneshot(Request::builder().uri("/orders/3").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body, json!({
            "id": 3,
            "table_no": 14,
            "item_name": "takoyaki",
            "duration": 6
        }));
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_get_non_existant_order(pool: PgPool) {
        let app = app(pool).await;
        let response = app
        .oneshot(Request::builder().uri("/orders/99").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_delete_table(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/tables/14")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_delete_non_existing_table(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/tables/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_delete_order(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/orders/3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[sqlx::test(fixtures("orders"))]
    async fn test_delete_non_existing_order(pool: PgPool) {
        let app = app(pool).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::DELETE)
                    .uri("/orders/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
