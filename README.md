# ShelfLife
An easy to use Rust application that automatically spins down and expires unused OKD projects

This is an application I worked on during the CSH summer hackathon. It's used to increase the ease of managment of old OKD projects on CSH's Openshift cluster. We don't have a good way of managing these applications, so here it is!

Currently, this app just queries the Openshift API for a particular project's info and then queries a mongodb server for data, but it's got a few nifty features planned:
  - Run once daily via a cronjob
  - Track all OKD namespaces, their admins, and when they were last deployed (or one of a few other timestamps, if those don't exist)
    - Store metrics on Openshift project lifespans
  - Notify admins of those namespaces when their namespace hits a certain age (according to the build timestamp)
  - Spin down, back up, and delete old projects to save resources

It will also have an interactive mode for setup, configuration, and monitoring.

## Usage
To use this app, you'll need a few things:
  An Openshift cluster (duh) that can be accessed via API calls
  An admin™ account for ShelfLife to view and manage namespaces
  MongoDB installed and running
  A .env file to store Openshift cluster information, DB information, and a few other miscillaneous things in this format:
```
#Openshift API stuff
export OKD_TOKEN="<api-token>"
export ENDPOINT="<endpoint-address>:<port>"

#mongodb stuff
export DB_ADDR="<mongodb-address>"
export DB_PORT="<mongodb-port>"

# misc
export TEST_PROJECT="<namespace-to-query>"
```

## CLI Arguments
To run a query on a known namespace, run with the `n` flag
