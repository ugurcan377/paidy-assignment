# Paidy Rust Challange SimpleRestaurantApi

## Running the app and tests

For running the API you would need a postgreSQL database, and set the connection string for it
in the environment variable `DATABASE_URL`. For convenience you can use the one provided docker compose file with

```
docker compose up
```

The `DATABASE_URL` for the postgreSQL in the container would be

```
export DATABASE_URL="postgres://paidy:paidy@localhost:5435/"
```

Note that it uses non-standard port `5435` for preventing collusion with existing postgreSQL servers.

After the setup tests can be run (assuming you have a cargo environment ready) with

```
cargo test
```

and the API itself can be run with

```
cargo run
```

If you didn't want to export `DATABASE_URL` you can also use it like this

```
DATABASE_URL="postgres://paidy:paidy@localhost:5435/" cargo test
```

If you want the app in a more production ready fashion you can use the `prod` profile in the docker compose with

```
docker compose --profiles prod up
```

It will compile and place the api server binary inside a container, ready to use without any rust environment necessary

## API Documentation

### GET

- `/tables`: List all orders across all tables
- `/tables/:id`: List all orders with the given table number `:id`
- `/orders/:id`: Get a specific order with the order id `:id`

### POST

- `/tables/:id`: Add one or more orders to a given table with number `:id`
  - Returns the list of all orders for that table number
  - Example JSON body
```
{
    "orders": [
        {
            "item_name": "karaage",
        },
        {
            "item_name": "yakisoba",
            "duration": 10 
            // Optional: if empty it will assign a random duration between 5 and 15
        }
    ]
}
```
### DELETE

- `/tables/:id`: Delete all orders for a given table number `:id`
- `/orders/:id`: Delete a specific order with the order id `:id`

### Example request
```
> curl --header "Content-Type: application/json" -X POST --data '{"orders": [{"item_name": "karaage"}, {"item_name": "yakisoba", "duration": 10}]}' http://localhost:4000/tables/14

{
    "orders": [
        {
            "id": 3,
            "table_no": 14,
            "item_name": "karaage",
            "duration": 14
        },
        {
            "id": 4,
            "table_no": 14,
            "item_name": "yakisoba",
            "duration": 10
        }
    ]
}
```

## Assumptions and Simplifications

* Uses static randomly assigned durations as allowed in the problem definition
* Assumed tables always exists and any integer can be used as a table number
* When adding orders any empty elements in the order list will be ignored instead of returning and error message
* Couldn't managed to make a usual schema validation work within the given time frame, it relies on HTTP codes instead of proper error messages
* The sqlx library didn't have bulk insert support and relied on postgreSQL tricks to work, went with the simple but inefficient make separate queries for creating each item route.