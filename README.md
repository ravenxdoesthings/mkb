# mkb

EVE Online microkillboard

Running:

```bash
docker compose up -d

MKB_ESI_APPLICATION_ID=<app id from esi> \
MKB_ESI_APPLICATION_SECRET=<app secret from esi> \
MKB_ESI_REDIRECT_URI=<your hostname>/auth/callback \
MKB_DATABASE_URI=postgres://localhost:5432/postgres \
		cargo run
```
