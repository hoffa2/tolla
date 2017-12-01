#!/bin/sh
# Docker entrypoint (pid 1), run as root
[ "$1" = "mongod" ] || exec "$@" || exit $?

# Make sure that database is owned by user mongodb
[ "$(stat -c %U /data/db)" = mongodb ] || chown -R mongodb /data/db

exec /root/app &

mkdir -p /etc/mongodb/ssl
chmod 700 /etc/mongodb/ssl
touch /config/pemkey.crt
cat /config/keys.pem /config/certificate.pem > /config/pemkey.crt
cp /config/CAcert.pem /etc/mongodb/ssl/
cp /config/pemkey.crt /etc/mongodb/ssl/

chown -R mongodb:mongodb /etc/mongodb
ls -l /config
# Drop root privilege (no way back), exec provided command as user mongodb
cmd=exec; for i; do cmd="$cmd $i"; done
exec su -s /bin/sh -c "$cmd" mongodb
