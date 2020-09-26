#!/usr/bin/python
# coding: utf-8

#
# Send mp4 streaming media URIs to Samsung Smart TV for immediate playback
#

import os, socket, argparse, logging, subprocess, cgi, httplib, StringIO, urllib2, re, urlparse

#
# Function to discover services on the network using SSDP
# Inspired by https://gist.github.com/dankrause/6000248
#

class SsdpFakeSocket(StringIO.StringIO):
     def makefile(self, *args, **kw): return self

def ssdp_discover(service):
    group = ("239.255.255.250", 1900)
    message = "\r\n".join(['M-SEARCH * HTTP/1.1', 'HOST: {0}:{1}', 'MAN: "ssdp:discover"', 'ST: {st}','MX: 3','',''])
    socket.setdefaulttimeout(0.5)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
    sock.sendto(message.format(*group, st=service), group)
    results = []
    while True:
        try:
            r = httplib.HTTPResponse(SsdpFakeSocket(sock.recv(1024))) 
            r.begin()
            results.append(r.getheader("location"))
        except socket.timeout:
            break
    return results

#
# DIDL-Lite template
# Note that this is included in the Universal Plug and Play (UPnP) message in urlencoded form
#

didl_lite_template = """<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:sec="http://www.sec.co.kr/">
   <item id="f-0" parentID="0" restricted="0">
      <upnp:class>object.item.videoItem</upnp:class>
      <res protocolInfo="http-get:*:video/mp4:DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000" sec:URIType="public">$$$URI$$$</res>
   </item>
</DIDL-Lite>"""

# Remove newlines and whitespace
didl_lite = ' '.join(cgi.escape(didl_lite_template.replace("\n","")).split())

#
# Universal Plug and Play (UPnP) message templates.
# Note that the request times out if there is a blank charater between the header lines and the message
# or if we do not use .replace("\n", "\r\n") to get "CRLF line terminators"
#

AVTransportTemplate = """POST $$$APIURL$$$ HTTP/1.1
Accept: application/json, text/plain, */*
Soapaction: "urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI"
Content-Type: text/xml;charset="UTF-8"

<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>$$$URI$$$</CurrentURI>
      <CurrentURIMetaData>$DIDL</CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"""

PlayTemplate = """POST $$$APIURL$$$ HTTP/1.1
Accept: application/json, text/plain, */*
Soapaction: "urn:schemas-upnp-org:service:AVTransport:1#Play"
Content-Type: text/xml;charset="UTF-8"

<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"""

#
# Send message to the TV
#

def sendMessage(ip, port, message):
  s    = socket.socket(socket.AF_INET, socket.SOCK_STREAM, 0)
  s.connect((ip, port))
  sent = s.send(message.replace("\n", "\r\n"))
  if (sent <= 0):
    print("Error sending message")
    s.close()
    return
  recv = s.recv(100000)
  s.close()
  logging.debug(recv)
  logging.debug("")

#
# Parse command line and send messages to the TV
#

def main():
  parser = argparse.ArgumentParser(description='Send mp4 video streams to a Samsung Smart TV', add_help = True)
  flags = parser.add_argument_group('Arguments')
  parser.add_argument("-v", "--verbose", help="Verbose output, print requests and responses", action="store_true")
  flags.add_argument('uri', nargs='+', default = None, help = 'Required. URI to be sent to TV. If this does not start with http, it is sent to yt-downloader for processing.')

  args = parser.parse_args()
  if args.verbose:
    logging.basicConfig(level=logging.DEBUG)

  # Find TV devices using SSDP   
  tvs = []
  results = ssdp_discover("urn:schemas-upnp-org:service:AVTransport:1")
  for result in results:
    logging.debug(result)
    data = urllib2.urlopen(result).read()
    # logging.debug(data) 
    expr = re.compile(r"urn:upnp-org:serviceId:AVTransport.*<controlURL>(.*)</controlURL>", re.DOTALL)
    regexresult = expr.findall(data)
    logging.debug(regexresult)
    o = urlparse.urlparse(result)
    tv = {"ip": o.hostname, "port": o.port, "url": regexresult[0]}
    logging.debug(tv)
    tvs.append(tv)

  # Do a search if required
  if not (args.uri[0].startswith("http")):
    myexec = "youtube-dl"
    try:
      FNULL = open(os.devnull, 'w')
      subprocess.call([myexec, '--version'], stdout=FNULL, stderr=FNULL)
    except OSError:
      print "%s is not installed." % myexec
      print "Install it in order to be able to search YouTube."
      exit(1)
    command = ["youtube-dl", "-f", "mp4", "-g", "--default-search", "auto", " ".join(args.uri)]
    logging.debug(command)
    process = subprocess.Popen(command, stdout=subprocess.PIPE)
    out, err = process.communicate()
    logging.debug(out)
    args.uri = out.strip()
  else:
    args.uri = args.uri[0]
    
  for tv in tvs:
    message = AVTransportTemplate.replace("$DIDL", didl_lite).replace("$$$URI$$$", args.uri).replace("$$$APIURL$$$", tv["url"])
    logging.debug(message)
    sendMessage(tv["ip"], tv["port"], message)
    message = PlayTemplate.replace("$$$APIURL$$$", tv["url"])
    logging.debug(message)
    sendMessage(tv["ip"], tv["port"], message)
  
main()