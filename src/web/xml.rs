use crate::{config::AppConfig, database::MediaFile};

/// XML escape helper
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Get the server's IP address for use in URLs.
fn get_server_ip(config: &AppConfig) -> String {
    if config.server.interface != "0.0.0.0" && !config.server.interface.is_empty() {
        return config.server.interface.clone();
    }

    // Try to find a non-loopback IP address
    if let Ok(output) = std::process::Command::new("ip").arg("addr").output() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("inet ") && !line.contains("127.0.0.1") {
                if let Some(ip_part) = line.trim().split_whitespace().nth(1) {
                    if let Some(ip) = ip_part.split('/').next() {
                        return ip.to_string();
                    }
                }
            }
        }
    }
    
    "127.0.0.1".to_string() // Fallback
}

/// Get the appropriate UPnP class for a given MIME type.
fn get_upnp_class(mime_type: &str) -> &str {
    if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("audio/") {
        "object.item.audioItem"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem"
    } else {
        "object.item" // Generic item
    }
}

pub fn generate_description_xml(config: &AppConfig) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <device>
        <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
        <friendlyName>{}</friendlyName>
        <manufacturer>OpenDLNA</manufacturer>
        <modelName>OpenDLNA Server</modelName>
        <UDN>uuid:{}</UDN>
        <serviceList>
            <service>
                <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
                <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
                <SCPDURL>/ContentDirectory.xml</SCPDURL>
                <controlURL>/control/ContentDirectory</controlURL>
                <eventSubURL>/event/ContentDirectory</eventSubURL>
            </service>
        </serviceList>
    </device>
</root>"#,
        xml_escape(&config.server.name),
        config.server.uuid
    )
}

pub fn generate_scpd_xml() -> String {
    // This XML is static and doesn't need formatting.
    r#"<?xml version="1.0" encoding="UTF-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <actionList>
        <action>
            <name>Browse</name>
            <argumentList>
                <argument><name>ObjectID</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_ObjectID</relatedStateVariable></argument>
                <argument><name>BrowseFlag</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_BrowseFlag</relatedStateVariable></argument>
                <argument><name>Filter</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Filter</relatedStateVariable></argument>
                <argument><name>StartingIndex</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Index</relatedStateVariable></argument>
                <argument><name>RequestedCount</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>SortCriteria</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_SortCriteria</relatedStateVariable></argument>
                <argument><name>Result</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Result</relatedStateVariable></argument>
                <argument><name>NumberReturned</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>TotalMatches</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>UpdateID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_UpdateID</relatedStateVariable></argument>
            </argumentList>
        </action>
    </actionList>
    <serviceStateTable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ObjectID</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_BrowseFlag</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Filter</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Index</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Count</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_SortCriteria</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Result</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_UpdateID</name><dataType>ui4</dataType></stateVariable>
    </serviceStateTable>
</scpd>"#.to_string()
}

pub fn generate_browse_response(object_id: &str, files: &[MediaFile], config: &AppConfig) -> String {
    let server_ip = get_server_ip(config);
    let mut didl = String::from(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">"#);
    let mut number_returned = 0;
    let mut total_matches = 0;

    if object_id == "0" {
        // Root directory: show containers for media types
        didl.push_str(r#"<container id="video" parentID="0" restricted="1"><dc:title>Video</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="audio" parentID="0" restricted="1"><dc:title>Music</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="image" parentID="0" restricted="1"><dc:title>Pictures</dc:title><upnp:class>object.container</upnp:class></container>"#);
        number_returned = 3;
        total_matches = 3;
    } else {
        let (media_type_filter, parent_id) = match object_id {
            "video" => ("video/", "video"),
            "audio" => ("audio/", "audio"),
            "image" => ("image/", "image"),
            _ => ("", "0"),
        };

        if !media_type_filter.is_empty() {
            let filtered_files: Vec<_> = files
                .iter()
                .filter(|f| f.mime_type.starts_with(media_type_filter))
                .collect();
            
            number_returned = filtered_files.len();
            total_matches = filtered_files.len();

            for file in filtered_files {
                let file_id = file.id.unwrap_or(0);
                let url = format!("http://{}:{}/media/{}", server_ip, config.server.port, file_id);
                let upnp_class = get_upnp_class(&file.mime_type);

                didl.push_str(&format!(
                    r#"<item id="{id}" parentID="{parent_id}" restricted="1">
                        <dc:title>{title}</dc:title>
                        <upnp:class>{upnp_class}</upnp:class>
                        <res protocolInfo="http-get:*:{mime}:*" size="{size}">{url}</res>
                    </item>"#,
                    id = file_id,
                    parent_id = parent_id,
                    title = xml_escape(&file.filename),
                    upnp_class = upnp_class,
                    mime = &file.mime_type,
                    size = file.size,
                    url = xml_escape(&url)
                ));
            }
        }
    }
    didl.push_str("</DIDL-Lite>");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>{}</Result>
            <NumberReturned>{}</NumberReturned>
            <TotalMatches>{}</TotalMatches>
            <UpdateID>0</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>"#,
        xml_escape(&didl),
        number_returned,
        total_matches
    )
}