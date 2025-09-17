# Secret Sealer Service

This service exposes an API for sealing secrets.

## API specs
The API specs are defined in [./openapi.yaml].

## Env configuration
Available env variables are listed and described in [./env_config.hjson].

## Testing
Run the service with `cargo run`.

Move in `tests` folder and run an HTTP server (e.g. `python3 -m http.server`) and go to `localhost:8000`. There you can find a form and the sealed secret will be written in the console logs.
