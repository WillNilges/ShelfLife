#Install MongoDB
echo Installing MongoDB...
wget -qO - https://www.mongodb.org/static/pgp/server-4.2.asc | sudo apt-key add -
echo "deb [ arch=amd64 ] https://repo.mongodb.org/apt/ubuntu bionic/mongodb-org/4.2 multiverse" | sudo tee /etc/apt/sources.list.d/mongodb-org-4.2.list
sudo apt-get update
sudo apt-get install -y mongodb-org
sudo systemctl enable mongod && sudo systemctl start mongod

echo Installing Rust Toolchain...
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

echo Install build deps
sudo apt install -y build-essential libssl-dev pkg-config

git clone http://www.github.com/willnilges/shelflife
cd shelflife
cp .env.sample .env
echo Repo cloned! Please fill out the .env file!

# TODO: I wanted to add somehting else here... I don't remember what that was.
# AHA! TODO: CHRON JOB CONFIG! Ask the user what they want
