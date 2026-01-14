#!/bin/bash
: "${CONTEXT:=do-nyc3-beeb}"
kubectl get secret --context $CONTEXT -n dorch $1 -o json \
    | jq .data.$2 \
    | xargs echo \
    | base64 -d
