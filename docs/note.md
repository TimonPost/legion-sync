## Systems

| System | Client | Server| Feature |
| :----- | :----- | :----- |  :----- |
| `insert_received_entities_system` |   | X |   | 
| `track_modifications_system`      | X |   |   |
|`authoritative_system`             |   | X |   |      
|       | 	|   |   |
| `tcp_client_receive_system`       | X	|   | TCP (tcp-tranport)  |
| `tcp_client_sent_system`          | X	|   | TCP (tcp-tranport)  |
| `tcp_connection_listener`         | 	| X | TCP (tcp-tranport)  |
| `tcp_server_receive_system`       | 	| X | TCP (tcp-tranport)  |
| `tcp_server_sent_system`          |   | X | TCP (tcp-tranport)  |  

## Resources

| Resource | Client | Server| Feature |
| :----- | :----- | :----- |  :----- |
| BufferResource                | X | X | |            	
| EventResource                 | X | X | |        	
| Packer                        | X | X | |    	
| PostBoxResource               | X| | |            	
| PostOfficeResource            | | X | |                	
| RegisteredComponentsResource  | X | X | |         
| TrackResource                 | X | X | |        
|                               | | | |
| TcpClientResource             | X | | TCP (tcp-tranport) |            	
| TcpListenerResource           | | X | TCP (tcp-tranport) |

## Entity Insert

1. Client Inserts Entity with own generated entity id.
2. Client sends Entity Insert command to server.
3. Server receives Entity Insert command.
4. Server reserves server id for to insert Entity.
4. Authoritative Server check for entity insert
5. Server Inserts Entity and attaches server generated entity id.
6. Entity Insert acknowlegement to client.
7. Client receives acknowlegement.
8. Client replaces client generated entity id with server generated id.

Problems:
1. Prevent legion fiering events when performing actions are performed by the library on main world.


# Rename refactor:
- entity_id / uid
- uid_allocator / allocator / alloc
- changes/modifications/difference

- Encapsulation Check
- Documentatie Round
- new() => default()


