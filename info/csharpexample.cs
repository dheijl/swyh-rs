 /// <summary>
 /// Gets the absolute URL for a service
 /// </summary>
 /// <param name="device">UPnP device to control</param>
 /// <param name="service">UPnP service</param>
 /// <returns>the absolute URL for the service</returns>
 protected Uri GetControlUri(Device device, Service service)
 {
     var requestUri = new Uri(service.ControlURL, UriKind.RelativeOrAbsolute);
     if (!requestUri.IsAbsoluteUri || requestUri.IsFile) // In Mono.Android, requestUri is not a relative uri but a file
     {
         requestUri = new Uri(new Uri(device.URLBase), requestUri);
     }
     return requestUri;
 }
 


 /// <summary>
 /// Play a media HTTP link to a media renderer
 /// </summary>
 /// <param name="mediaRenderer">media renderer</param>
 /// <param name="uri">uri of the media</param>
 /// <returns>Task</returns>
 public async Task Play(Device mediaRenderer, string uri)
 {
     await Play(mediaRenderer, new MediaInfo() { Uri = uri });
 }

/// <summary>
/// Play a media HTTP link to a media renderer
/// </summary>
/// <param name="mediaRenderer">media renderer</param>
/// <param name="mediaInfo">informations about the media</param>
/// <returns>Task</returns>
public async Task Play(Device mediaRenderer, MediaInfo mediaInfo)
{
	using (var httpClient = new HttpClient())
	{
		httpClient.DefaultRequestHeaders.ExpectContinue = false;

		mediaInfo = await CheckUri(httpClient, mediaInfo);

		var requestUri = new Uri(mediaRenderer.Services.First(service => service.ServiceType == "urn:schemas-upnp-org:service:AVTransport:1").ControlURL, UriKind.RelativeOrAbsolute);
		if (!requestUri.IsAbsoluteUri || requestUri.IsFile) // In Mono.Android, requestUri will not a relative uri but a file
		{
			requestUri = new Uri(new Uri(mediaRenderer.URLBase), requestUri);
		}

		var xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>" +
			"<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">" +
				"<s:Body>" +
					"{0}" +
				"</s:Body>" +
			"</s:Envelope>";

		var xmlContent = String.Format(xml, String.Format(
			"<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">" +
				"<InstanceID>0</InstanceID>" +
				"<CurrentURI>{0}</CurrentURI>" +
				"<CurrentURIMetaData>{1}</CurrentURIMetaData>" +
			"</u:SetAVTransportURI>", mediaInfo.Uri,
			String.Format(WebUtility.HtmlEncode(
			"<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:sec=\"http://www.sec.co.kr/\">" +
				"<item id=\"f-0\" parentID=\"0\" restricted=\"1\">" +
				(mediaInfo.Title == null ? String.Empty : "<dc:title>{3}</dc:title>") +
				(mediaInfo.Author == null ? String.Empty : "<dc:creator>{4}</dc:creator>") +
				"<upnp:class>object.item.{2}</upnp:class>" +
				"<res protocolInfo=\"http-get:*:{1}:DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000\">{0}</res>" +
				"</item>" +
				"</DIDL-Lite>"), mediaInfo.Uri, mediaInfo.Type, GetMimeTypeUPnPClass(mediaInfo.Type), mediaInfo.Title, mediaInfo.Author)));

		var request = CreateContent("SetAVTransportURI", xmlContent);
		request.Headers.Add("transferMode.dlna.org", "Streaming");
		request.Headers.Add("contentFeatures.dlna.org", "DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000");
		var response = await httpClient.PostAsync(requestUri, request);
		if (response.IsSuccessStatusCode)
		{
			xmlContent = String.Format(xml,
				"<u:Play xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">" +
					"<InstanceID>0</InstanceID>" +
					"<Speed>1</Speed>" +
				"</u:Play>");

			response = await httpClient.PostAsync(requestUri, CreateContent("Play", xmlContent));
		}
		else
		{
			throw new Exception(response.ReasonPhrase);
		}
	}
}
