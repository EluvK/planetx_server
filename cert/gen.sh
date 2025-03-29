#!/bin/bash

openssl genpkey -algorithm RSA -out private.key
openssl req -new -key private.key -out certificate.csr -subj /"/C=CN/ST=/L=/O=ELUVK/OU=DEV/CN="
openssl x509 -req -days 3650 -in certificate.csr -signkey private.key -out certificate.crt
