#!/bin/sh

# Construct the PostgreSQL URL
pg_url="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@${POSTGRES_HOST}:${POSTGRES_PORT}/${POSTGRES_DB}"
export pg_url

# Function to check if PostgreSQL is ready
wait_for_postgres() {
    echo "Waiting for PostgreSQL at $POSTGRES_HOST:$POSTGRES_PORT"
    # Check connectivity
    until PGPASSWORD="$POSTGRES_PASSWORD" pg_isready -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER"; do
        echo "PostgreSQL is not accepting connections yet. Waiting..."
        sleep 1
    done

    echo "PostgreSQL is accepting connections."

    # Check database responsiveness with a simple query
    until PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c '\q' >/dev/null 2>&1; do
        echo "PostgreSQL is not responsive yet. Waiting..."
        sleep 1
    done

    echo "PostgreSQL is ready and responsive!"
}

# Run the wait function
wait_for_postgres

/app/korotagger migrate

# Execute the main application with environment variables
exec /app/korotagger "$@"
