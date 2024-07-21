#!/bin/sh

# Construct the PostgreSQL URL
pg_url="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@${POSTGRES_HOST}:${POSTGRES_PORT}/${POSTGRES_DB}"
export pg_url

# Function to check if PostgreSQL is ready
wait_for_postgres() {
    echo "Waiting for PostgreSQL at ${POSTGRES_HOST}:${POSTGRES_PORT} to be ready..."

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

# Function to check if the application is running for the first time
check_first_time_run() {
    result=$(PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -tAc "SELECT to_regclass('config.first_time_run');")
    if [ "$result" = "config.first_time_run" ]; then
        entry_count=$(PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -tAc "SELECT COUNT(*) FROM config.first_time_run;")
        if [ "$entry_count" -eq 0 ]; then
            return 0  # First-time run
        else
            return 1  # Not a first-time run
        fi
    else
        return 0  # First-time run, as the table does not exist
    fi
}

# Run the wait function
wait_for_postgres

# Check if it is the first time run
if check_first_time_run; then
    echo "First-time run detected. Running SQL initialization scripts..."

    # Run SQL initialization scripts
    for sql_file in /app/migrations/*.sql; do
        if [ -f "$sql_file" ]; then
            echo "Running SQL script $sql_file..."
            PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -f "$sql_file"
        fi
    done
else
    echo "Not the first-time run. Skipping SQL initialization."
fi

# Execute the main application with environment variables
exec /app/korotagger "$@"
