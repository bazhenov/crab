# Release procedure

1. push tag into repo
   ```console
   $ git co master
   $ git tag x.x.x
   $ git push origin x.x.x
   ```
1. wait while GHA will [build a release](https://github.com/bazhenov/crab/actions)
1. update [homebrew descriptor](https://github.com/bazhenov/homebrew-tap/blob/master/crab.rb)
   * update `url`
   * update `sha256`
     ``` console
     $ curl -Ls https://github.com/bazhenov/crab/releases/download/x.x.x/crab-x.x.x-x86_64-apple-darwin.tar.gz | sha256sum
     ```
1. push homebrew descriptor to a registry
1. **check** if `brew install bazhenov/tap/crab` works correctly