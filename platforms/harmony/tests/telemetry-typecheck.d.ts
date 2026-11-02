// Ambient stand-ins for the system modules Telemetry.ets imports, so run.sh can type-check the adapter off a device. Everything here is any on purpose: the guard exists to catch Telemetry.ets calling a TelemetryPolicy member that no longer exists, which the --noCheck emit in telemetry-adapter.test.cjs cannot see.
declare module "@ohos.bundle.bundleManager" { const bundleManager: any; export default bundleManager; }
declare module "@ohos.hiviewdfx.hiAppEvent" {
  namespace hiAppEvent { type Watcher = any; type AppEventGroup = any; type ParamType = any; }
  const hiAppEvent: any;
  export default hiAppEvent;
}
declare module "@ohos.app.ability.errorManager" {
  namespace errorManager { type ErrorHandler = any; }
  const errorManager: any;
  export default errorManager;
}
declare module "@ohos.process" { const process: any; export default process; }
declare module "libmsimeclient.so" { const client: any; export default client; }
