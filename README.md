Pull contracts: `git subtree pull --prefix=contracts contracts main --squash`
Push contracts: `git subtree push --prefix=contracts contracts main`

```
DEV:
docker compose          \         
  -f code/sql/docker-compose.yml \
  -f code/sql/docker-compose.local.yml \                 
  up -d
```