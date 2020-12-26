# ShelfLife

An easy to use Rust application that automatically spins down and expires unused
OKD projects.

This is an application I worked on during the CSH summer hackathon. It's used
to increase the ease of management of old OKD projects on CSH's Openshift
cluster. We don't have a good way of managing these applications, so here it is!

At its core, this tool queries the Openshift API for a particular project's
info, queries a MongoDB server for data, and then reacts to that data.

Basic features:

* Operate on a cron-based schedule
* Track all OKD namespaces, their admins, and when they were last deployed
  (or one of a few other timestamps, if those don't exist)
  * Store metrics on Openshift project lifespans in MongoDB
* Notify admins of those namespaces when their namespace hits a certain age
  (according to the build timestamp)
* Spin down, back up, and delete old projects to save resources

[Also check out the frontend!](https://github.com/willnilges/shelflife-frontend) (In development)

## Installation

Recommended distro: Ubuntu 18.04

To use this application, download the install.sh file and run it as root. It will fetch the latest release binary, as well as a `env` file, if you don't have one, and install your crontab. From there, you must fill out the .env file with your OKD cluster info, a mongodb, mailing info and options, a backup path, and a log path.

## Usage

ShelfLife uses cronjobs to complete its tasks. The default looks like this:

```
0 * * * 1-3 /usr/local/bin/shelflife -a #On the hour, every hour, Sat-Wed,
0 * * * 6-7 /usr/loca/bin/shelflife -a  #query the OKD cluster for changes

0 12 * * 4 /usr/local/bin/shelflife -D  # On Thursday at noon, do a dryrun of the 
                                        # actions to be taken, and let the
                                        # admins know what is about to happen

0 12 * * 5 /usr/local/bin/shelflife -C  # On Friday at noon, do a cull,
                                        # and send a report of the cull to
                                        # the admins.
```

To use the shelflife command line, run `shelflife` and pass it flags:

### CLI Arguments

```
USAGE:
    shelflife [FLAGS] [OPTIONS]

FLAGS:
    -a, --all                   Queries all available namespaces and adds/updates any that are missing/outdated to the
                                database.
    -c, --cull                  Checks graylist for projects that need attention. Takes appropriate course of action.
    -C, --cull_with_report      Culls, and generates and sends a report to ShelfLife admins.
    -d, --dryrun                Checks graylist for projects that need attention. Takes no action.
    -D, --dryrun_with_report    Dryruns, and generates and sends a report to ShelfLife admins.
    -h, --help                  Prints help information
    -l, --list                  Print namespaces currently tracked in the database.
    -V, --version               Prints version information
    -w, --whitelist             Enables whitelist mode for that command, performing operations on the whitelist instead
                                of the greylist.

OPTIONS:
    -k, --known <NAMESPACE>      Query API and ShelfLife Database for a known namespace. If it is missing from the
                                 database, the user is asked if they want to add it.
    -p, --project <NAMESPACE>    Query API for project info about a namespace.
    -r, --remove <NAMESPACE>     Removes a namespace from the database.
```

## Contributing

Firstly, I just wanna say, "Thanks!" 

Next, here's what you'll need:

### Prerequisites 

* An Openshift cluster that can be accessed via API calls
* An admin™ account for ShelfLife to view and manage namespaces
* MongoDB installed and running (https://docs.mongodb.com/manual/tutorial/install-mongodb-on-ubuntu/)
* A .env file to store Openshift cluster information, DB information, and a few
  other miscellaneous things. Get started by copying the provided `.env.sample`
  file to `.env` and then fill in the appropriate values.


### Installation

#### Build Dependencies

- Ubuntu (Server 18.04 recommended)
- `build-essential`
- `libssl-dev`
- `pkg-config`

- Clone the repo and run the install script.
```
git clone https://www.github.com/willnilges/shelflife
./dev_install.sh
```

- Run the following commands on your openshift cluster:

```
oc create sa shelflife-dev-bot # Create a service account for shelflife to use.
oc adm policy add-cluster-role-to-user cluster-admin system:serviceaccount:default:shelflife-dev-bot # Make the service account an admin on your cluster.
oc get token shelflife-dev-bot # Spits out the API token.
```

- Copy the API token, cluster URL, and email credentials into the .env file.
- Configure the .env file with your email information.
- Go to town.

