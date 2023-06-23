FROM archlinux:latest

COPY\
  data/polkadot\
  data/rococo.json.fork-off\
  ./

RUN chmod u+x polkadot

ENTRYPOINT ./polkadot --unsafe-rpc-external --rpc-cors all --rpc-methods unsafe --tmp --chain rococo.json.fork-off --alice
