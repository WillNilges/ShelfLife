#!/bin/bash

set -e

RELEASE_REPO=https://api.github.com/repos/willnilges/shelflife/releases/latest

get_file () {
curl -s "$RELEASE_REPO" \
	| grep "$1" \
	| cut -d : -f 2,3 \
	| tr -d \" \
	| wget -qi -
}

if [ "$UID" -ne 0 ]; then
	echo "Please run as root."
	exit 1
fi

echo "Install to /usr/local/bin/shelflife"
mkdir /usr/local/bin/shelflife \
	|| echo "Directory already exists. I guess this'll be an upgrade."
cd /usr/local/bin/shelflife

echo "Downloading binary..."
get_file shelflife
get_file env.sample
if ! test -f ".env"; then
	mv env.sample .env
fi
chmod +x shelflife

# TODO: Don't overwrite crontab settings
echo "Edit crontab"
crontab -l > mycron_tmp

echo "
0 * * * 1-3 /usr/local/bin/shelflife -a
0 * * * 6-7 /usr/loca/bin/shelflife -a
0 12 * * 4 /usr/local/bin/shelflife -D
0 12 * * 5 /usr/local/bin/shelflife -C
" >> mycron_tmp
crontab mycron_tmp
rm mycron_tmp

echo "Done. Please fill out the .env file in /usr/local/bin/shelflife"
