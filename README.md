# Ripple 2.0
> let's ripple better

## How to run?

1. >$git submodule update --init --recursive
2. Before running this command if you already have a `~/.ripple` folder take backup.
>./ripple init
3. Open the Plugin Manifest file `~/.ripple/firebolt-extn-manifest.json`
Update the __*default_path*__
```
    "default_path": "[Path to your workspace]/ripple-workspace/target/debug/",
```
Update __*default_extension*__ value will be `dylib` for mac, `dll` for windows and for unix it will `so`
```
"default_extension": "dylib",
```
4. Open the Device Manifest file `~/.ripple/firebolt-device-manifest.json`
Update  __*default_extension*__
```
"library": "~/.ripple/firebolt-app-library.json",
```
5. Open the App library file `~/.ripple/firebolt-app-library.json`

Add the below parameter to the `start_page` in the app library. Replace [app_id] with actual app id
```
__firebolt_endpoint=ws%3A%2F%2F10.0.0.107%3A3473%3FappId%3D[app_id]%26session%3D[app_id]
```

For eg for refui of firebolt cert app
```
default_library": [
        {
          "app_id": "refui",
          ....
          "start_page": "https://firecertapp.firecert.comcast.com/prod/index.html?systemui=true&__firebolt_endpoint=ws%3A%2F%2F10.0.0.107%3A3473%3FappId%3Drefui%26session%3Drefui&systemui=true",
```

4. Find the ip address of the device which is connected to the same network router as the machine running Ripple.
> ripple run {ip address of the device} 

Note: Device should be accessible bothways between the machine which is running ripple and target device.
## Folder structure

Ripple folder structure has the below layers

#### Core folder
This folder contains the workspaces for 
1. `sdk` - Contains the building block for all ripple components. More info here.
2. `main` - Ripple main starter application loads the extensions, starts the gateway and its services.
3. `launcher`- Contains the Launcher code extension which uses Thunder RDKShell API for launching apps. Ripple can run without this extension for external launchers.

#### Examples folder
This folder contains the workspaces which solves usecase with actual examples
1. `rpc_extn` - Provides an example of how a firebolt method can be made into an extension using the sdk. This would be applicable for both Proprietary and Device specific extensions.
2. `thunder_extn` - Provides an example of how a Thunder method which might be Proprietary or bound to a device type be made into an extension.


### What is Ripple magic sauce?

Load extensions(Dynamic shared libraries) during Ripple Startup
![Ripple startup](./docs/images/RippleStartup.jpeg)

Lets apply this to an actual Ripple 2.0 runtime which has loaded the below plugins
1. `device/thunder`: This starts the thunder thread and accepts Device Requests. It also accepts Device extentions which assist in proprietary thunder plugins and device specific thunder extensions.
2. `examples/rpc_extn`: This provides 2 extensions one for externalizing hdcp firebolt api and another to showcase support for legacy which can call into another rpc extension in `core/main`
3. `examples/thunder_extn`: Provides an extn for HDCPProfile Thunder api which exists in streaming devices and not on display devices like TV.



Breakdown the big Ripple monolith into smaller runtime extensions using a standardized SDK.
![Ripple startup](./docs/images/R2DeliverySo.jpeg)

