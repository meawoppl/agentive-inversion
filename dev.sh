#!/usr/bin/env bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# PID files location
PID_DIR=".dev-pids"
BACKEND_PID="$PID_DIR/backend.pid"
FRONTEND_PID="$PID_DIR/frontend.pid"
LOG_DIR=".dev-logs"

print_header() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  Agentive Inversion - Development Environment${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

check_deps() {
    local missing=()

    if ! command -v cargo &> /dev/null; then
        missing+=("cargo (Rust)")
    fi

    if ! command -v trunk &> /dev/null; then
        missing+=("trunk (cargo install trunk)")
    fi

    if ! command -v diesel &> /dev/null; then
        missing+=("diesel_cli (cargo install diesel_cli --no-default-features --features postgres)")
    fi

    if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
        missing+=("wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        echo -e "${RED}Missing dependencies:${NC}"
        for dep in "${missing[@]}"; do
            echo -e "  ${YELLOW}•${NC} $dep"
        done
        echo ""
        return 1
    fi
    return 0
}

check_env() {
    if [ ! -f .env ]; then
        echo -e "${YELLOW}No .env file found. Creating from .env.example...${NC}"
        if [ -f .env.example ]; then
            cp .env.example .env
            echo -e "${YELLOW}Please edit .env with your configuration.${NC}"
            return 1
        else
            echo -e "${RED}.env.example not found!${NC}"
            return 1
        fi
    fi

    # Check for required env vars
    source .env
    if [ -z "$DATABASE_URL" ]; then
        echo -e "${RED}DATABASE_URL not set in .env${NC}"
        return 1
    fi
    return 0
}

is_running() {
    local pid_file=$1
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            return 0
        fi
    fi
    return 1
}

start_services() {
    print_header
    echo ""

    echo -e "${BLUE}Checking dependencies...${NC}"
    if ! check_deps; then
        echo -e "${RED}Please install missing dependencies and try again.${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ All dependencies installed${NC}"
    echo ""

    echo -e "${BLUE}Checking environment...${NC}"
    if ! check_env; then
        exit 1
    fi
    echo -e "${GREEN}✓ Environment configured${NC}"
    echo ""

    # Create directories
    mkdir -p "$PID_DIR" "$LOG_DIR"

    # Check if already running
    if is_running "$BACKEND_PID"; then
        echo -e "${YELLOW}Backend already running (PID: $(cat $BACKEND_PID))${NC}"
    else
        echo -e "${BLUE}Starting backend...${NC}"
        source .env
        cargo run --bin backend > "$LOG_DIR/backend.log" 2>&1 &
        echo $! > "$BACKEND_PID"
        echo -e "${GREEN}✓ Backend started (PID: $(cat $BACKEND_PID))${NC}"
    fi

    if is_running "$FRONTEND_PID"; then
        echo -e "${YELLOW}Frontend already running (PID: $(cat $FRONTEND_PID))${NC}"
    else
        echo -e "${BLUE}Starting frontend...${NC}"
        cd crates/frontend
        trunk serve --proxy-backend=http://127.0.0.1:3000/api/ > "../../$LOG_DIR/frontend.log" 2>&1 &
        echo $! > "../../$FRONTEND_PID"
        cd ../..
        echo -e "${GREEN}✓ Frontend started (PID: $(cat $FRONTEND_PID))${NC}"
    fi

    echo ""
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}  Development environment started!${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo -e "  ${BLUE}Frontend:${NC}  http://localhost:8080"
    echo -e "  ${BLUE}Backend:${NC}   http://localhost:3000"
    echo -e "  ${BLUE}API:${NC}       http://localhost:3000/api"
    echo ""
    echo -e "  ${YELLOW}Logs:${NC}"
    echo -e "    Backend:  $LOG_DIR/backend.log"
    echo -e "    Frontend: $LOG_DIR/frontend.log"
    echo ""
    echo -e "  ${YELLOW}Commands:${NC}"
    echo -e "    ./dev.sh status  - Check service status"
    echo -e "    ./dev.sh logs    - Tail all logs"
    echo -e "    ./dev.sh stop    - Stop all services"
    echo ""
}

stop_services() {
    print_header
    echo ""

    local stopped=0

    if is_running "$BACKEND_PID"; then
        echo -e "${BLUE}Stopping backend...${NC}"
        kill $(cat "$BACKEND_PID") 2>/dev/null || true
        rm -f "$BACKEND_PID"
        echo -e "${GREEN}✓ Backend stopped${NC}"
        stopped=1
    fi

    if is_running "$FRONTEND_PID"; then
        echo -e "${BLUE}Stopping frontend...${NC}"
        kill $(cat "$FRONTEND_PID") 2>/dev/null || true
        rm -f "$FRONTEND_PID"
        echo -e "${GREEN}✓ Frontend stopped${NC}"
        stopped=1
    fi

    # Also kill any orphaned processes
    pkill -f "trunk serve" 2>/dev/null || true
    pkill -f "target/debug/backend" 2>/dev/null || true

    if [ $stopped -eq 0 ]; then
        echo -e "${YELLOW}No services were running${NC}"
    fi

    echo ""
}

show_status() {
    print_header
    echo ""

    echo -e "${BLUE}Service Status:${NC}"
    echo ""

    if is_running "$BACKEND_PID"; then
        local pid=$(cat "$BACKEND_PID")
        echo -e "  Backend:  ${GREEN}● Running${NC} (PID: $pid)"
        echo -e "            ${BLUE}http://localhost:3000${NC}"
    else
        echo -e "  Backend:  ${RED}○ Stopped${NC}"
    fi

    if is_running "$FRONTEND_PID"; then
        local pid=$(cat "$FRONTEND_PID")
        echo -e "  Frontend: ${GREEN}● Running${NC} (PID: $pid)"
        echo -e "            ${BLUE}http://localhost:8080${NC}"
    else
        echo -e "  Frontend: ${RED}○ Stopped${NC}"
    fi

    echo ""

    # Check API health if backend is running
    if is_running "$BACKEND_PID"; then
        echo -e "${BLUE}API Health:${NC}"
        if curl -s http://localhost:3000/health > /dev/null 2>&1; then
            echo -e "  Health check: ${GREEN}● Healthy${NC}"
        else
            echo -e "  Health check: ${YELLOW}● Starting...${NC}"
        fi
        echo ""
    fi
}

show_logs() {
    print_header
    echo ""
    echo -e "${BLUE}Tailing logs (Ctrl+C to exit)...${NC}"
    echo ""

    if [ ! -d "$LOG_DIR" ]; then
        echo -e "${YELLOW}No logs found. Start services first with: ./dev.sh start${NC}"
        exit 1
    fi

    tail -f "$LOG_DIR"/*.log 2>/dev/null || echo -e "${YELLOW}No log files found${NC}"
}

run_migrations() {
    print_header
    echo ""

    echo -e "${BLUE}Checking environment...${NC}"
    if ! check_env; then
        exit 1
    fi

    echo -e "${BLUE}Running database migrations...${NC}"
    source .env
    diesel migration run
    echo -e "${GREEN}✓ Migrations complete${NC}"
    echo ""
}

db_status() {
    print_header
    echo ""

    echo -e "${BLUE}Checking environment...${NC}"
    if ! check_env; then
        exit 1
    fi

    source .env
    echo -e "${BLUE}Database Status:${NC}"
    echo ""

    # Check connection
    echo -n "  Connection: "
    if psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Connected${NC}"
    else
        echo -e "${RED}✗ Cannot connect${NC}"
        echo ""
        return 1
    fi

    # Show database info
    echo ""
    echo -e "${BLUE}Tables:${NC}"
    psql "$DATABASE_URL" -c "\dt" 2>/dev/null | head -20 || echo -e "${YELLOW}Could not list tables${NC}"

    echo ""
    echo -e "${BLUE}Record Counts:${NC}"
    for table in todos categories emails email_accounts; do
        count=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM $table" 2>/dev/null | tr -d ' ' || echo "?")
        printf "  %-20s %s\n" "$table:" "$count"
    done

    echo ""
}

db_nuke() {
    print_header
    echo ""

    echo -e "${RED}WARNING: This will delete ALL data in the database!${NC}"
    echo ""

    read -p "Are you sure you want to continue? (type 'yes' to confirm): " confirm
    if [ "$confirm" != "yes" ]; then
        echo -e "${YELLOW}Aborted.${NC}"
        return 1
    fi

    echo -e "${BLUE}Checking environment...${NC}"
    if ! check_env; then
        exit 1
    fi

    source .env

    echo ""
    echo -e "${BLUE}Reverting all migrations...${NC}"

    # Count migrations
    migration_count=$(ls -d migrations/*/ 2>/dev/null | wc -l)

    # Revert all migrations
    for i in $(seq 1 $migration_count); do
        diesel migration revert 2>/dev/null || break
    done

    echo -e "${BLUE}Re-running all migrations...${NC}"
    diesel migration run

    echo ""
    echo -e "${GREEN}✓ Database has been reset${NC}"
    echo ""
}

db_seed() {
    print_header
    echo ""

    echo -e "${BLUE}Checking environment...${NC}"
    if ! check_env; then
        exit 1
    fi

    source .env

    if [ ! -f seed-data.sql ]; then
        echo -e "${RED}seed-data.sql not found${NC}"
        return 1
    fi

    echo -e "${BLUE}Loading seed data...${NC}"
    psql "$DATABASE_URL" -f seed-data.sql

    echo ""
    echo -e "${GREEN}✓ Seed data loaded${NC}"
    echo ""
}

show_help() {
    print_header
    echo ""
    echo -e "${BLUE}Usage:${NC} ./dev.sh <command>"
    echo ""
    echo -e "${BLUE}Service Commands:${NC}"
    echo "  start      Start backend and frontend services"
    echo "  stop       Stop all running services"
    echo "  restart    Restart all services"
    echo "  status     Show status of all services"
    echo "  logs       Tail logs from all services"
    echo ""
    echo -e "${BLUE}Database Commands:${NC}"
    echo "  db:status  Show database connection and table stats"
    echo "  db:migrate Run database migrations"
    echo "  db:seed    Load seed data from seed-data.sql"
    echo "  db:nuke    Reset database (revert and re-run all migrations)"
    echo ""
    echo -e "${BLUE}Other Commands:${NC}"
    echo "  check      Check dependencies and environment"
    echo "  help       Show this help message"
    echo ""
    echo -e "${BLUE}Examples:${NC}"
    echo "  ./dev.sh start      # Start development environment"
    echo "  ./dev.sh status     # Check what's running"
    echo "  ./dev.sh logs       # Watch the logs"
    echo "  ./dev.sh db:status  # Check database"
    echo "  ./dev.sh stop       # Stop everything"
    echo ""
}

check_all() {
    print_header
    echo ""

    echo -e "${BLUE}Checking dependencies...${NC}"
    echo ""

    # Check individual tools
    echo -n "  cargo:          "
    if command -v cargo &> /dev/null; then
        echo -e "${GREEN}✓ $(cargo --version)${NC}"
    else
        echo -e "${RED}✗ Not installed${NC}"
    fi

    echo -n "  trunk:          "
    if command -v trunk &> /dev/null; then
        echo -e "${GREEN}✓ $(trunk --version 2>/dev/null || echo 'installed')${NC}"
    else
        echo -e "${RED}✗ Not installed (cargo install trunk)${NC}"
    fi

    echo -n "  diesel_cli:     "
    if command -v diesel &> /dev/null; then
        echo -e "${GREEN}✓ $(diesel --version 2>/dev/null || echo 'installed')${NC}"
    else
        echo -e "${RED}✗ Not installed (cargo install diesel_cli --no-default-features --features postgres)${NC}"
    fi

    echo -n "  wasm target:    "
    if rustup target list --installed | grep -q wasm32-unknown-unknown; then
        echo -e "${GREEN}✓ wasm32-unknown-unknown${NC}"
    else
        echo -e "${RED}✗ Not installed (rustup target add wasm32-unknown-unknown)${NC}"
    fi

    echo ""
    echo -e "${BLUE}Checking environment...${NC}"
    echo ""

    echo -n "  .env file:      "
    if [ -f .env ]; then
        echo -e "${GREEN}✓ Found${NC}"
    else
        echo -e "${YELLOW}! Not found (will be created from .env.example)${NC}"
    fi

    echo -n "  DATABASE_URL:   "
    if [ -f .env ]; then
        source .env
        if [ -n "$DATABASE_URL" ]; then
            # Mask the password in output
            masked=$(echo "$DATABASE_URL" | sed 's/:\/\/[^:]*:[^@]*@/:\/\/***:***@/')
            echo -e "${GREEN}✓ $masked${NC}"
        else
            echo -e "${RED}✗ Not set${NC}"
        fi
    else
        echo -e "${YELLOW}! .env not found${NC}"
    fi

    echo ""
}

# Main command handler
case "${1:-}" in
    start)
        start_services
        ;;
    stop)
        stop_services
        ;;
    restart)
        stop_services
        sleep 1
        start_services
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs
        ;;
    db:status)
        db_status
        ;;
    db:migrate|migrate)
        run_migrations
        ;;
    db:seed)
        db_seed
        ;;
    db:nuke)
        db_nuke
        ;;
    check)
        check_all
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        show_help
        exit 1
        ;;
esac
