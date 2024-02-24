<pre>
████████▄  ███▄▄▄▄      ▄████████      ████████▄     ▄████████  ▄██████▄     ▄███████▄    ▄███████▄    ▄████████    ▄████████
███   ▀███ ███▀▀▀██▄   ███    ███      ███   ▀███   ███    ███ ███    ███   ███    ███   ███    ███   ███    ███   ███    ███
███    ███ ███   ███   ███    █▀       ███    ███   ███    ███ ███    ███   ███    ███   ███    ███   ███    █▀    ███    ███
███    ███ ███   ███   ███             ███    ███  ▄███▄▄▄▄██▀ ███    ███   ███    ███   ███    ███  ▄███▄▄▄      ▄███▄▄▄▄██▀
███    ███ ███   ███ ▀███████████      ███    ███ ▀▀███▀▀▀▀▀   ███    ███ ▀█████████▀  ▀█████████▀  ▀▀███▀▀▀     ▀▀███▀▀▀▀▀   
███    ███ ███   ███          ███      ███    ███ ▀███████████ ███    ███   ███          ███          ███    █▄  ▀███████████
███   ▄███ ███   ███    ▄█    ███      ███   ▄███   ███    ███ ███    ███   ███          ███          ███    ███   ███    ███
████████▀   ▀█   █▀   ▄████████▀       ████████▀    ███    ███  ▀██████▀   ▄████▀       ▄████▀        ██████████   ███    ███
███    ███  
</pre>

## What is it?
DNSDropper is a tool for anyone looking for a light-weight dns proxy with filtering capabilities. Like blocking ads! It works by being a proxy in-between you and your normal DNS service, filtering any DNS requests for domains in your black list.

## How to configure
DNSDropper uses in a single configuration file that is divided up by major features: server, udp_proxy, and logging. You can find a breakdown of each feature below.

| Section Name |            Description            |                    Breakdown Link                    |
|:------------:|:---------------------------------:|:----------------------------------------------------:|
|    server    | Configure the DNSDropper runtime. |  [server breakdown](docs/config.section.server.md)   |
|  udp_proxy   |     Configure DNS filtering.      | [server breakdown](docs/config.section.udp_proxy.md) |
|   logging    |     Configure logging output.     |  [server breakdown](docs/config.section.logging.md)  |

You can also find examples of different configurations under the ```test/``` folder.


## How to use (standard configuration)
1. Configure your ```server.yaml``` to fit your needs, and run the service.
   1. To specify the config directory, use the ```--config``` or ```-c``` argument. e.g. ```dns_dropper --config config/myconfig.yaml```
   2. ```config/server.yaml``` is the default if no arguments are provided.
2. Configure your machines DNS to point to the locally running dns_dropper.
3. All DNS requests should now be filtered to your specification!

## How to develop
// TODO

## How to contribute
// TODO

