#/bin/bash

project=$1

login=$2

endpoint=$3

oc login https://$endpoint --token=$login

mkdir $project

oc project $project

oc get -o yaml --export all > $project/project.yaml

echo DONE WITH GET for export all

for object in rolebindings serviceaccounts secrets imagestreamtags podpreset cms egressnetworkpolicies rolebindingrestrictions limitranges resourcequotas pvcs templates cronjobs statefulsets hpas deployments replicasets poddisruptionbudget endpoints
do
  oc get -o yaml --export $object > $project/$object.yaml
  echo DONE WITH GET for export $object
done

exit 0
