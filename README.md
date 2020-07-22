# Deploying

Bump the version in `Cargo.toml`.

```
cargo make deploy-flow
```

Update timetables - needs new implementation
```
ssh root@s4.njk.onl /app/transit-radar/update-timetables.sh
```

Logs 
```
kubectl logs -lapp=transit-radar
```
