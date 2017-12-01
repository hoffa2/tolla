package main

import (
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/asn1"
	"fmt"
	"io"
	"io/ioutil"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"sync/atomic"
	"time"
)

type TlsToDB struct {
	incoming *tls.Conn
	dbConn   *net.Conn
}

type Lease struct {
	certificate *x509.Certificate
	leaseStart  time.Time
}

type Proxy struct {
	config       *tls.Config
	serialNumber uint64
	portIncoming string
	dbAddr       string
	caAddr       string
	connPool     map[string]*tls.Conn
	leases       []*Lease
	httpClient   *http.Client
}

type Pipe struct {
	status int32
}

type OtherName struct {
	TypeID asn1.ObjectIdentifier
	Value  asn1.RawValue
}

func (l *Lease) Expired() bool {
	return time.Since(l.leaseStart).Seconds() > 10
}

func (p *Proxy) assertPermissions(intent string) error {
	var values url.Values

	values.Add("intent", "intent")
	values.Add("user", fmt.Sprintf("%d", p.serialNumber))

	url, err := url.Parse(p.caAddr)
	if err != nil {
		// Abandon Ship?
		return err
	}

	url.RawQuery = values.Encode()

	request, err := http.NewRequest("GET", url.String(), nil)
	if err != nil {
		return err
	}

	resp, err := p.httpClient.Do(request)
	if err != nil {
		return err
	}

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("Access denied")
	}

	return nil
}

func (p *Proxy) VerifyPeerCertificate(rawCerts [][]byte, verifiedChains [][]*x509.Certificate) error {
	var intentExt *pkix.Extension
	var clientCert *x509.Certificate

	// Brute-forcing my way to heaven
	for _, chain := range verifiedChains {
		for _, cert := range chain {
			for _, ext := range cert.Extensions {
				oid := asn1.ObjectIdentifier{2, 5, 29, 17}
				if oid.Equal(ext.Id) {
					clientCert = cert
					intentExt = &ext
					break
				}
			}
		}
	}

	if intentExt == nil {
		return fmt.Errorf("No intent present in client certificate")
	}

	for i, lease := range p.leases {
		if lease.certificate.Equal(clientCert) {
			if !lease.Expired() {
				return nil
			} else {
				p.leases = append(p.leases[:i], p.leases[i+1:]...)
			}
		}
	}

	var err error
	var v asn1.RawValue
	var rest []byte
	if rest, err = asn1.Unmarshal(intentExt.Value, &v); err != nil {
		return fmt.Errorf("failed to marshal extension")
	}
	fmt.Println(v.Tag)

	if v.Tag != 16 {
		fmt.Println(v.Tag)
		return fmt.Errorf("No othername present")
	}

	rest = v.Bytes
	if rest, err = asn1.Unmarshal(rest, &v); err != nil {
		return fmt.Errorf("failed to marshal extension")
	}

	if !v.IsCompound {
		return fmt.Errorf("x509: failed to unmarshal GeneralName.otherName: not compund")
	}

	fmt.Println(v.Tag)
	if v.Tag != 0 {
		fmt.Println(v.Tag)
		return fmt.Errorf("No othername present")
	}

	var other OtherName

	v.FullBytes = append([]byte{}, v.FullBytes...)
	v.FullBytes[0] = asn1.TagSequence | 0x20

	if _, err = asn1.Unmarshal(v.FullBytes, &other); err != nil {
		return fmt.Errorf("failed to marshal extension")
	}

	var intent string
	if _, err = asn1.Unmarshal(other.Value.Bytes, &intent); err != nil {
		return fmt.Errorf("failed to marshal extension")
	}

	err = p.assertPermissions(intent)
	if err != nil {
		return err
	}

	p.leases = append(p.leases, &Lease{certificate: clientCert, leaseStart: time.Now()})

	return nil
}

func (p *Proxy) handleIncoming() error {
	l, err := tls.Listen("tcp", p.portIncoming, p.config)
	if err != nil {
		return err
	}

	for {
		conn, err := l.Accept()
		if err != nil {
			log.Println(err)
		}
		tlsCon := conn.(*tls.Conn)

		fmt.Println("Accepting")

		err = tlsCon.Handshake()
		log.Println(err)

		go func(c *tls.Conn) {
			p.handleConnection(c)
		}(tlsCon)
	}
}

func (p *Pipe) done(side int32, dst, src net.Conn) {
	if atomic.AddInt32(&p.status, side) == 0 {
		dst.Close()
		src.Close()
	}
}

func (p *Proxy) handleConnection(src *tls.Conn) {
	dst, err := net.Dial("tcp", fmt.Sprintf("localhost:%s", p.dbAddr))
	if err != nil {
		log.Println(err)
		return
	}

	quit := &Pipe{
		status: 0,
	}

	pipe := func(src, dst net.Conn, side int32) {
		io.Copy(dst, src)
		quit.done(side, dst, src)
	}

	go pipe(src, dst, 1)
	go pipe(dst, src, -1)
}

func main() {
	portIncoming := os.Getenv("LISTEN_ADDR")
	if portIncoming == "" {
		panic("LISTEN_ADDR not present")
	}
	dbAddr := os.Getenv("DB_ADDR")
	if dbAddr == "" {
		panic("DB_ADDR not present")
	}
	caAddr := os.Getenv("CA_ADDR")
	if caAddr == "" {
		panic("CA_ADDR not present")
	}
	pemFolder := os.Getenv("PEM_FOLDER")
	if pemFolder == "" {
		panic("PEM_FOLDER not present")
	}

	cert, err := tls.LoadX509KeyPair(fmt.Sprintf("%s/%s", pemFolder, "certificate.pem"),
		fmt.Sprintf("%s/%s", pemFolder, "keys.pem"))
	if err != nil {
		log.Println(err)
		os.Exit(1)
	}

	caCert, err := ioutil.ReadFile(fmt.Sprintf("%s/%s", pemFolder, "CAcert.pem"))
	if err != nil {
		log.Println(err)
		os.Exit(1)
	}

	clientCertPool := x509.NewCertPool()
	clientCertPool.AppendCertsFromPEM(caCert)

	proxy := &Proxy{
		serialNumber: cert.Leaf.SerialNumber.Uint64(),
		portIncoming: portIncoming,
		dbAddr:       dbAddr,
		caAddr:       caAddr,
		connPool:     make(map[string]*tls.Conn),
	}
	config := &tls.Config{VerifyPeerCertificate: proxy.VerifyPeerCertificate,
		ClientAuth: tls.RequireAndVerifyClientCert,
		ClientCAs:  clientCertPool, Certificates: []tls.Certificate{cert}}

	proxy.config = config

	err = proxy.handleIncoming()
	log.Println(err)
}
