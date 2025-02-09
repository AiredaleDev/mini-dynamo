# Mini-Dynamo -- A small, DynamoDB-like distributed database.

Another project written for fun. Presently unmaintained.

## Building and running

This database is comprised of three executables -- manager, store, and client. The client accepts requests over a CLI, and the storage nodes and managers communcate
directly with the client.

In the previous (and private) implementation of this database, I used the manager node as a proxy for the whole system, where the manager node would bottleneck
all communication. I wanted to see if performance improves if clients could communicate directly with storage nodes.

Since this was originally just a class project, the original system assumed manager nodes could never go down. I would like to relax this assumption in the final version of this tool.
