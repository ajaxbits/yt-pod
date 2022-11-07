use chrono::{Duration, NaiveDate};
use rss::{
    extension::{
        itunes::{ITunesItemExtension, ITunesItemExtensionBuilder},
        ExtensionBuilder,
    },
    GuidBuilder, Item, ItemBuilder,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct PodInfo {
    id: rss::Guid,
    url: String,
    episode: Option<i32>,
    title: String,
    duration_str: String,
    duration_secs: i64,
    author: String,
    date: String,
    link: String,
    description: String,
}

impl PodInfo {
    fn update_ep_number(self, number: i32) -> Self {
        PodInfo {
            episode: Some(number),
            ..self
        }
    }
}

impl From<Item> for PodInfo {
    fn from(item: Item) -> Self {
        let itunes_info = item.itunes_ext().expect("item had no itunesextension");
        PodInfo {
            id: item.guid().expect("could not find id").to_owned(),
            url: item.link().expect("could not find link").to_owned(),
            episode: itunes_info.episode().map(|value| {
                value
                    .parse::<i32>()
                    .expect(&format!("could not parse {value} as i32"))
            }),
            title: item
                .title()
                .expect("could not find title in feed")
                .to_owned(),
            duration_str: itunes_info
                .duration()
                .map(|v| v.to_owned())
                .expect("could not find duration for this episode"),
            duration_secs: item.enclosure()
                .map(|enc| enc.length())
                .map(|s| s.parse::<i64>().expect(&format!("could not parse {s} as i64")))
                .expect("could not compute duration_secs from the enclosure for the specified rss entry"),
            author: item.author().expect("could not find author for specified episode").to_owned(),
            date: item.pub_date().expect("could not find date for specified episode").to_owned(),
            link: item.link().expect("could not find link for specified episode").to_owned(),
            description: item.description().expect("could not find description for specified episode").to_owned(),
        }
    }
}

impl From<youtube_dl::SingleVideo> for PodInfo {
    fn from(video: youtube_dl::SingleVideo) -> Self {
        let duration = video
            .duration
            .expect(&format!("could not find a duration for {}", video.id))
            .as_i64()
            .expect("could not convert duration to i64");
        let duration = Duration::seconds(duration);

        let date = NaiveDate::parse_from_str(
            &video
                .upload_date
                .expect(&format!("Could not find an upload_date for {}", video.id)),
            "%Y%m%d",
        )
        .expect(&format!(
            "could not parse video {}'s upload date as str",
            video.id,
        ));

        fn gen_description(description: Option<String>) -> String {
            // "From the creators of the original Fallout games comes Arcanum. A fantasy RPG about the dynamics between technology and magic, and the endless despicability of gnomes.\nSupport the channel at: https://www.patreon.com/mandaloregaming or https://www.paypal.me/MandaloreGaming\nI take video suggestions at mandaloremovies@gmail.com\nTwitter: https://twitter.com/Lord_Mandalore\nBig thanks to @Branzoles for his artistic contributions.\n00:00 - Intro\n00:37 - Issues & Fixes\n01:50 - The World of Arcanum\n05:18 - Game Intro\n06:44 - Visuals\n07:53 - Music & Sound Design\n12:12 - Gameplay Mechanics\n24:42 - Questing & Story\n26:46 - Story (SPOILERS)\n48:42 - Conclusions\n50:04 - Credits\n51:28 - Baah\n\n#Arcanum #ArcanumReview #ArcanumPC #Troika",
            if let Some(description) = description {
                let description: String = description
                    .split("\n")
                    .into_iter()
                    .map(|line| {
                        format!(
                            "<p>{}</p>",
                            html_escape::encode_text_to_string(line, &mut String::new())
                        )
                    })
                    .collect();
                println!("{}", description.clone());
                description
            } else {
                String::new()
            }
        }

        fn gen_duration_str(duration: Duration) -> String {
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() - (&duration.num_hours() * 60);
            let seconds = duration.num_seconds() - (&duration.num_minutes() * 60);

            let time: Vec<String> = vec![hours, minutes, seconds]
                .into_iter()
                .map(|number| format!("{number}"))
                .map(|time_str| match time_str.chars().count() {
                    1 => format!("0{time_str}"),
                    _ => time_str,
                })
                .collect();

            format!("{}:{}:{}", time[0], time[1], time[2])
        }

        PodInfo {
            id: GuidBuilder::default()
                .value(&video.id)
                .permalink(false)
                .build(),
            url: format!("https://0d1f-73-45-179-121.ngrok.io/{}", &video.id),
            episode: None,
            title: video.title,
            duration_str: gen_duration_str(duration),
            duration_secs: duration.num_seconds(),
            author: video
                .uploader
                .expect(&format!("Could not find an uploader for {}", video.id)),
            date: format!("{}", date.format("%a, %d %b %Y 00:00:00 +0000")), // ex: Thu, 06 Oct 2022 15:07:56 +0000
            link: video
                .webpage_url
                .expect(&format!("Could not find a url for {}", video.id)),
            description: gen_description(video.description),
        }
    }
}

impl From<PodInfo> for rss::Item {
    fn from(ep: PodInfo) -> Self {
        let enclosure: rss::Enclosure = rss::EnclosureBuilder::default()
            .mime_type("audio/m4a".to_owned())
            .length(ep.duration_secs.to_string())
            .url(ep.url)
            .build();

        let episode = Some(ep.episode.map(|ep| ep.to_string()));

        let itunes_metadata: ITunesItemExtension = ITunesItemExtensionBuilder::default()
            .episode(ep.episode.map(|ep| ep.to_string()))
            .author(Some(ep.author))
            .duration(Some(ep.duration_str))
            .block(Some("Yes".to_string()))
            .build();

        // We have to write a whole custom extension just to get <itunes:title>
        // TODO this is magic. Figure out how it works. Like what the heck are the
        // first entries in the BTreeMap about??
        let itunes_title = BTreeMap::from([(
            "itunes_title".to_owned(),
            vec![ExtensionBuilder::default()
                .name("itunes:title".to_owned())
                .value(Some(ep.title.clone()))
                .build()],
        )]);

        let item: Item = ItemBuilder::default()
            .guid(Some(ep.id))
            .pub_date(Some(ep.date))
            .title(Some(ep.title))
            .extensions(BTreeMap::from([("itunes_title".to_owned(), itunes_title)])) // put <itunes:title> in there
            .itunes_ext(Some(itunes_metadata))
            .enclosure(Some(enclosure))
            .link(Some(ep.link))
            .description(Some(ep.description))
            .build();

        item
    }
}