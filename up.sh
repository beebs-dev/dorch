#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/"
KUBECONTEXT="${KUBECONTEXT:-do-nyc3-beeb}"
NAMESPACE="${NAMESPACE:-dorch}"
echo "Using kubectl context: $KUBECONTEXT"
do_build() {
    build_args=()
        for arg in "$@"; do
            case "$arg" in
            party-router)
                build_args+=("party")
                ;;
            *)
                build_args+=("$arg")
                ;;
        esac
    done
    ./build.sh --push "${build_args[@]}"
}
do_restart() {
    restart_args=()
    restart_app=$([ "${#restart_args[@]}" -eq 0 ])
    restart_server=$([ "${#restart_args[@]}" -eq 0 ])
    for arg in "$@"; do
        case "$arg" in
        party)
            restart_args+=("party")
            restart_args+=("party-router")
            ;;
        client)
            restart_app=true
            ;;
        server|proxy)
            restart_server=true
            ;;
        *)
            restart_args+=("$arg")
            ;;
        esac
    done
    if [ "$restart_app" = true ]; then
        kubectl rollout restart deployment --context $KUBECONTEXT -n apps apps-zandronum
    fi
    if [ "$restart_server" = true ]; then
        kubectl delete pod --context $KUBECONTEXT -n $NAMESPACE test-game
    fi
    kubectl rollout restart deployment --context $KUBECONTEXT -n $NAMESPACE "${restart_args[@]/#/$NAMESPACE-}"
}
main() {
    do_build "$@"
    kubectl --context $KUBECONTEXT apply -f crds/
    do_restart "$@"
    k9s -n $NAMESPACE --splashless --context $KUBECONTEXT
}
main "$@"