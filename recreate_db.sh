
docker compose down
docker system prune -af
docker compose up -d db
sleep 1
./diesel_run.sh

