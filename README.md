# Deploying

Cross build for linux:
```
cargo build --release --target x86_64-unknown-linux-musl  
```

Build frontend:
```
(cd seed-frontend && cargo make compile_release)
```

Deploy backend change:
```
ssh root@s4.njk.onl /app/transit-radar/service.sh stop
scp target/x86_64-unknown-linux-musl/release/webserver_sync root@s4.njk.onl:/app/transit-radar/transit-radar
scp VBB_Colours.csv root@s4.njk.onl:/app/transit-radar/
ssh root@s4.njk.onl /app/transit-radar/service.sh start
```

Deploy frontend change:
```
scp -r seed-frontend/pkg root@s4.njk.onl:/app/transit-radar/frontend-alpha/
scp -r seed-frontend/static root@s4.njk.onl:/app/transit-radar/frontend-alpha/
scp -r seed-frontend/index.html root@s4.njk.onl:/app/transit-radar/frontend-alpha/
scp -r seed-frontend/storybook.html root@s4.njk.onl:/app/transit-radar/frontend-alpha/
# presentation
scp -r seed-frontend/pres.html root@s4.njk.onl:/app/transit-radar/frontend-alpha/
```

Deploy ops:
```
scp ops/run.sh ops/service.sh ops/update-timetables.sh root@s4.njk.onl:/app/transit-radar/
scp -r ops/sites/* root@s4.njk.onl:/etc/nginx/sites-available/transit-radar/
ssh root@s4.njk.onl nginx -t && ssh root@s4.njk.onl nginx -s reload
```

Update timetables:
```
ssh root@s4.njk.onl /app/transit-radar/update-timetables.sh
```

Start
```
ssh root@s4.njk.onl /app/transit-radar/service.sh start
```

Stop
```
ssh root@s4.njk.onl /app/transit-radar/service.sh stop
```

Restart
```
ssh root@s4.njk.onl /app/transit-radar/service.sh restart
```

Logs 
```
ssh root@s4.njk.onl tail -f /var/log/transit-radar/log
```
