This directory contains a udp server that can interact with a local [sense hat](https://www.raspberrypi.org/products/sense-hat/) on a raspberry pi.  

Messages look like:

echo "quit" | nc -u -w0 $ip $port # kills the server
echo "clear" | nc -u -w0 $ip $port # kills the server
echo "1,2,3,4,5" | nc -u -w0 $ip $port # sets pixel (1,2) to [r, g, b] value [3, 4, 5]

Refer to the [sense hat python API docs](https://pythonhosted.org/sense-hat/api/).
