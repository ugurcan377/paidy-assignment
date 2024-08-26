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
