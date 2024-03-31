# Proxy Options

## Overview
This document covers all server options available for the DnsDropper config file.

## Options

|        Property        |     Type     | Description                                                                                                                                                                                                                                                                                           | Example                            |
|:----------------------:|:------------:|:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|:-----------------------------------|
|      **timeout**       |   Integer    | Choose the proxy timeout(ms).                                                                                                                                                                                                                                                                         | ```2000```                         |
|        **bind**        |    String    | Choose the bind of the udp proxy. Most of the time it will either be 0.0.0.0:53 for external and 127.0.0.1:53 for loopback configurations. The port number *must* be included in the definition.                                                                                                      | ```127.0.0.1:53```                 |
|     **dns_hosts**      | List(String) | Specify any number of DNS hosts you want to proxy DNS requests to.  The port number *must* be included in each definition.                                                                                                                                                                            | ```["8.8.8.8:53", "1.1.1.1:53"]``` |
| **domain_block_lists** | List(String) | Specify any number of resources to grab block lists from. It can be any type of resource url (http, file, etc.). The content of these resources *must* be in the format ```<ip>\s<domain>\n```. Domains loaded from multiple resources that contain the same domains will be deduplicated on startup. | ```["8.8.8.8:53", "1.1.1.1:53"]``` |

## Example Configurations

### 
```yaml
...
udp_proxy:
  
  # Set a timeout of 2000ms
  timeout: 2000
  
  # Bind a loopback to the current machine on port 53.
  bind: "127.0.0.1:53"
  
  # Set the DNS destinations
  dns_hosts:
    - "8.8.8.8:53"
    - "8.8.4.4:53"
    - "1.0.0.1:53"
    - "1.1.1.1:53"
      
  # Add an external and local resource for our block list.
  domain_block_lists:
    - https://raw.githubusercontent.com/blocklistproject/Lists/master/ads.txt
    - test/config/local_block_list.txt
...
```