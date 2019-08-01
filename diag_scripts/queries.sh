#!/bin/bash

# The best way to run this is
# ./queries.sh <namespace> > <namespace>_queries_output.txt && less <namespace>_queries_output.txt 

TOKEN=
ENDPOINT=
NAMESPACE=$@

echo PROJECT INFO:
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/oapi/v1/projects/$NAMESPACE

echo NAMESPACE INFORMATION: 
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/api/v1/namespaces/$NAMESPACE

echo PODS INFORMATION:
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/api/v1/namespaces/$NAMESPACE/pods

echo BUILD INFORMATION: 
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/apis/build.openshift.io/v1/namespaces/$NAMESPACE/builds

echo DEPLOYMENT INFORMATION: 
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/apis/apps/v1beta1/namespaces/$NAMESPACE/deployments

echo DEPLOYMENTCONFIGS INFORMATION
curl -k \
    -H "Authorization: Bearer $TOKEN" \
    -H 'Accept: application/json' \
    https://$ENDPOINT/apis/apps.openshift.io/v1/namespaces/$NAMESPACE/deploymentconfigs

echo ROLEBINDINGS INFORMATION: 
curl -k \
     -H "Authorization: Bearer $TOKEN" \
     -H 'Accept: application/json' \
     https://$ENDPOINT/apis/authorization.openshift.io/v1/namespaces/$NAMESPACE/rolebindings
