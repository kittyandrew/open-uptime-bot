# Open Uptime Bot
An open-source backend for uptime status or other physical signals, allowing to setup notifications and alerts.

### @TODOs
- [] More data in ntfy/telegram notifications:
  - [] Generating 24h/7d/1m data graphs
    - [] _Maybe_ overlaying with some schedules (import from DTEK)
    - [] Web-view for ntfy so it can be viewed not only in telegram
  - [] Summary in notifications (e.g. was online/down for ... etc)

# Configuring NTFY
  
Create admin user to use the API as:
```bash
# This will prompt you for the password.
ntfy user add --role=admin <name>
# Example (you can also pass pasword as env var):
# NTFY_PASSWORD=mysecret ntfy user add --role=admin catmin
```
Then, create token for your admin user:
```bash
# Example: ntfy token add catmin
ntfy token add <name>
```
Now store that token into your `.env`:
```bash
NTFY_ADMIN_TOKEN=<token>
```
  
Finally custom tier with some updated values for the users.
```bash
ntfy tier add \
  --name="basic" \
  --message-limit=1000 \
  --message-expiry-duration=24h \
  --reservation-limit=0 \
  --attachment-file-size-limit=100M \
  --attachment-total-size-limit=1G \
  --attachment-expiry-duration=12h \
  --attachment-bandwidth-limit=5G \
  open-uptime-bot-basic
```
Note that by default this tier is already added into `.env`, so it will work already (unless you changed its codename):
```bash
NTFY_USER_TIER=open-uptime-bot-basic
```

# Deployment with docker
  
```bash
nix develop # to initiate the dev shell
nix build .#docker # creates 'result' artifact in the current dir
docker load < result # load docker image into local docker registry
```
