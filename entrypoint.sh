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

# Function to check if the application is running for the first time
check_first_time_run() {
     # this will confirm if "member.required_roles" exist which is the last executed on members.sql
     check=$(PGPASSWORD="$POSTGRES_PASSWORD" psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -tAc "SELECT 1 FROM information_schema.tables WHERE table_schema = 'member' AND table_name = 'required_roles';")
     if [ "$check" = "1" ]; then
         return 1 # Not a First-time run
     else
         return 0 # Not a first-time run
     fi
}

# Run the wait function
wait_for_postgres

# Check if it is the first time run
if check_first_time_run; then
    echo "First-time run detected. Running SQL initialization scripts..."
    /app/korotagger migrate
else
    echo "Not the first-time run. Skipping SQL initialization."
fi

# Execute the main application with environment variables
exec /app/korotagger "$@"
