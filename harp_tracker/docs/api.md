## API for extensions system

The API should involve having a dedicated port in which users can send POST requests to the server. The format of the requests should be the following:

```
POST /
Content-Type: application/json

{
    "ConnectionName": string,
    "lat": float,
    "lon": float,
    "alt": float,
    "last_updated": int, // Unix timestamp
}
```

The program should then take in these values and add it into the datastream.

