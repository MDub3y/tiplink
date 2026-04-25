#!/bin/bash

for i in 1 2 3
do
  echo "---------------------------------------"
  echo "🚀 Building MPC Cluster $i..."
  
  # 1. Create the Cluster Hardware
  kind create cluster --config "clusters/mpc-$i.yaml" --name "mpc-$i"
  
  # 2. Switch Context
  kubectl config use-context "kind-mpc-$i"
  
  # 3. Apply the Database
  kubectl apply -f manifests/db-setup.yaml
  kubectl apply -f manifests/postgres.yaml
  
  echo "✅ Cluster $i is initialized with a private DB."
done

echo "---------------------------------------"
echo "🎉 Infrastructure ready: 3 isolated MPC nodes created."