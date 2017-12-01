FROM golang:1.8 as proxy
WORKDIR /root
COPY proxy/main.go .
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o app .

FROM alpine:edge
RUN \
apk add --no-cache mongodb && \
rm /usr/bin/mongoperf

VOLUME /data/db
EXPOSE 8080
EXPOSE 27017

WORKDIR /root/
COPY run.sh .
COPY --from=proxy /root/app .
ENTRYPOINT [ "/root/run.sh" ]
CMD ["mongod", "--sslMode requireSSL", "--sslPEMKeyFile /etc/mongodb/ssl/pemkey.crt", "--sslCAFile /etc/mongodb/ssl/CAcert.pem"]
