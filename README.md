# Zero to Production in Rust

## 3. Sign up new subscriber

### 3.4. Our First integration test

- black box testing: HTTP client like `reqwest`

1. embedded test module

- good for testing private structs

2. external tests foler

- good for integration test at identical level of using crate

3. doc test

## 3.5. Implementing our first integration test

- Test should be decoupled from app aside of objective target
- We need to run our App as a background task

  - tokio::spawn takes a future and hands it over to the runtime for polling, without waiting for its completion

#### 3.5.1.2 Choosing A Random Port

- port 0: OS scans available port to bind to app
- `std::net::TCpListener`: returns listener with ip, port info

## 3.7. Working with HTML Forms

- can try parameterized test with `rstest` crate

### 3.7.2 Capturing Our requirement with as tests

- roll-my-own parametrised test stops as soon as one test fail + we don't know which it was

#### 3.7.3.1 Extractors

- can extract url-encoded data from req body or send url-encoded data as res
- 10 extractors per handler fn

### 3.7.3.2 `From` and `FromRequest`

```rs
impl<T> FromRequest for Form<T>
where
    T: DeserializeOwned + 'static,
{
    type Error = actix_web::Error;
    async fn from_request(req: &HttpRequest, payload: &mut Payload) -> Result<Self, Self::Error> {
        // Omitted stuff around extractor configuration (e.g. payload size limits)
        match UrlEncoded::new(req, payload).await {
            Ok(item) => Ok(Form(item)),
            // The error handler can be customised.
            // The default one will return a 400, which is what we want. Err(e) => Err(error_handler(e))
        }
    }
}
```

- `UrlEncoded` does serde

```rs
serde_urlencoded::from_bytes::<T>(&body).map_err(|_| UrlencodedError::Parse)
```

##### 3.7.3.3.2 Efficiently

- thanks to `monomorphization` even with generics, serde is not slow
- all information required to ser/de a specific type are available at `compile_time`
- Josh Mcguigan: `Understanding Serde`

#### 3.7.3.4. Putting Everything Together

```rs
#[derive(serde::Deserialize)] pub struct FormData {
    email: String,
    name: String,
}

// Let's start simple: we always return a 200 OK
async fn subscribe(_form: web::Form<FormData>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
```

- before calling `subscribe`, `Form::from_request` deserialize body into FormData
- if `Form::from_request` fails, 400 BAD REQUEST, else 200 OK

## 3.8 Storing Data: Databases

### 3.8.1 Choosing A Database

- Postgres: exhaustive docs, easy to run locally and CI via Docker, well-supported within the Rust ecosystem

### 3.8.2 Choosing A Database Crate

1. crates

- tokio-postgres
- sqlx
- diesel

#### 3.8.2.1 compile-time safety

- When do we realize mistake?
  - disel and sqlx detect most of mistakes at compile-time
    - disel: CLI rust code gen
    - sqlx: proc macro to connect DB at compile-time + query validation

#### 3.8.2.2 SQL-first VS a DSL for query building

- disel support query builder (DSL)

#### 3.8.2.3 async VS sync interface

- Threads are for working in parallel, Async is for waiting in parallel
- sqlx, tokio-postgres support async
- disel supports only sync

| Crate          | Compile-time safety | Query interface | Async |
| -------------- | ------------------- | --------------- | ----- |
| tokio-postgres | N                   | SQL             | Y     |
| sqlx(chosen)   | Y                   | SQL             | Y     |
| diesel         | Y                   | DSL             | N     |

### 3.8.4 Database Setup

```
cargo install --version="~0.6" sqlx-cli --no-default-features --features rustls,postgres
```

- init_db.sh
- Cargo.toml

```toml
[dependencies.sqlx]
version = "0.6"
default-features = false
features = [
  "runtime-tokio-rustls",
  "macros",
  "postgres",
  "uuid",
  "chrono",
  "migrate",
]
```

- PgConnection
- organize files

```
src/
  configuration.rs
  lib.rs
  main.rs
  routes/
    mod.rs
    health_check.rs
    subscriptions.rs
  startup.rs
```

##### 3.8.5.2.2 Reading A Configuration File

```rs
// src/configuration.rs
pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let settings = config::Config::builder()
        .add_source(config::File::new("configuration", config::FileFormat::Yaml))
        .build()?;

    settings.try_deserialize::<Settings>()
}
```

### 3.9.2 actix-web Workers

- web::Data wraps connection in `Atomic Reference Counted pointer`

### 3.9.3 The Data Extractor

- the `web::Data<T>` cast any value to the type `T` (equivalent to dependency injection)

```rs
// src/routes/subscriptions.rs
pub async fn subscribe(
    _form: web::Form<FormData>,
    _connection: web::Data<PgConnection>,
```

### 3.9.4 The INSERT Query

- replace PgConnection to PgPool for sharing mut ref

```rs
// src/main.rs
let connection_pool = PgPool::connect(&configuration.database.connection_string())
    .await
    .expect("Failed to connect to Postgres.");
..
// src/startup.rs
pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);
    let server = HttpServer::new(move || {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(db_pool.clone())
..
// src/routes/subscriptions.rs
pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
    sqlx::query!(..)
    .execute(pool.get_ref())
    .await;
```

## 3.10 Updating Out Tests

```rs
async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");

    let server = run(listener, connection_pool.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }
}
```

### 3.10.1 Test Isolation

- Solutions

1. wrap the whole test in a SQL transaction and rollback at the end of it
   - no way to capture that connectino in a SQL tx context
2. spin up a brand-new logical database for each integration test
   - slower but easier
   1. create a new logical db with a unique name
   2. run db migrations on it.

```rs
// tests/health_check.rs
configuration.database.database_name = uuid::Uuid::new_v4().to_string();
let connection_pool = configure_database(&configuration.database).await;
..
async fn configure_database(config: &zero2prod::configuration::DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres.");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Failed to connect to Postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database.");

    connection_pool
}
```

# 4. Telemetry

## 4.1. Unknown Unknowns

### Known unknowns

- what happens if we lose connection to the database?
  - Does sqlx::PgPool try to automatically recover?
- what happens if an attacker tries to pass malicious payloads in the body of the POST like large payloads or SQL injection

### Unknown unknowns

- Sometimes experience is enough to transform an unknown unknown into a known unknown
- impossible to reproduce outside of live environment
  - the system is pushed outside of its usual operating conditions
  - multiple components experience failures at the same time
  - a change is introduced that moves the system equilibrium(e.g. tuning a retry policy)
  - no changes have been introduced for a long time (e.g. app have not been restarted for weeks and memory leaks)

## 4.2. Observability

- Observability is about being able to ask arbitrary questions about your environment `without having to know ahead of time what you wanted to ask`

### To build an observable system

- instrument our app to collect high-quality telemetry data
- access to tools and systems to efficiently slice, dice and manipulate the data to find answers to our questions

## 4.3. Logging

### 4.3.1. The log Crate

1. trace: lowest level, extremely verbose
2. debug
3. info
4. warn
5. error: serious failures that might have user impact

```rs
fn fallible_operation() -> Result<String, String> { ... }

pub fn main() {
  match fallible_operation() {
    Ok(success) => {
      log::info!("Operation succeeded: {}", success);
    }
    Err(err) => {
      log::error!("Operation failed: {}", err);
    }
  }
}
```

### 4.3.2. actix-web's Logger Middleware

```rs
// src/routes/startup.rs
let server = HttpServer::new(move || {
    App::new()
        .wrap(Logger::default())
..
```

### 4.3.3. The Facade Pattern

- global decision that app are uniquely positioned to do => `log` crate
  - file, print, send to remote over HTTP(e.g. ElasticSearch)
  - it provides Log trait instead of how to record log

```rs
pub trait Log: Sync + Sned {
  fn enabled(&self, metadata: &Metadata) -> bool;
  fn log(&self, record: &Record);
  fn flush(&self);
}
```

- should call `set_logger` at main to use log records => use `env_logger`
- `env_logger` to print all log records to terminal
  - format: `[<timestamp> <level> <module path>] <log message>`

```toml
# Cargo.toml
[dependencies]
env_logger = "0.9"
```

```rs
// src/main.rs
async fn main() -> std::io::Result<()> {
// `init` does call `set_logger`, so this is all we need to do.
// We are falling back to printing all logs at info-level or above
// if the RUST_LOG environment variable has not been set.
env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
```

- print trace-level logs (default: RUST_LOG=info)

```sh
RUST_LOG=trace cargo run
```

## 4.4 Instrumenting POST /subscriptions

- add log as a dependency

```toml
#! Cargo.toml
[dependencies]
log = "0.4"
```

### 4.4.1 Interactions With External Systems

- success => log::info!()
- failure => log::error!()

### 4.4.2 Think Like A User

- make it observable like customer is reporting by email
- our log should include id (email) info

### 4.4.3 Log Mus Be Easy To Correlate

- add UUID to each log::info!

## 4.5 Structured Logging

### what should not do

- rewrite all upstream components in the req processing pipeline
- change the sign of all downstream fn.s calling from subscribe handler

### what should do

- each sub-routines has
  - duration
  - context

### then

- trying to represent tree-like processing pipeline
- Logs are the wrong abstraction

### 4.5.1 The `tracing` Crate

- expand upon logging-style diag with additional info

### 4.5.2 Migrating From `log` To `tracing`

### 4.5.3 `tracing`'s Span

```rs
let request_span = tracing::info_span!(
    "Adding a new subscriber",
    %request_id,
    subscriber_email = %form.email,
    subscriber_name = %form.name
);
let _request_span_guard = request_span.enter();
```

- info_span! gets multiple arguments => structured info
- `%` prefix: implement `Display` for logging
- `.enter()`: as long not dropped, all downstream spans and log events will be registered as children
  - like Rust' RAII(Resource Acquisition Is Initialization)
