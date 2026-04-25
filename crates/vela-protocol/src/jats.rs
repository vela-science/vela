//! JATS XML ingestion — parse Journal Article Tag Suite XML into structured paper data.
//!
//! JATS is the standard XML format used by PubMed Central, bioRxiv, and most publishers.
//! Parsing JATS gives us structured metadata for free: authors with ORCIDs, DOIs, structured
//! abstracts, section-level body text, figure captions, and citation lists.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::bundle::{Author, Extraction, Provenance};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct JatsPaper {
    pub title: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub pmc: Option<String>,
    pub authors: Vec<JatsAuthor>,
    pub journal: Option<String>,
    pub year: Option<i32>,
    pub abstract_text: String,
    pub abstract_sections: Vec<JatsSection>,
    pub body_sections: Vec<JatsSection>,
    pub keywords: Vec<String>,
    pub references: Vec<JatsReference>,
    pub figures: Vec<JatsFigure>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct JatsAuthor {
    pub name: String,
    pub orcid: Option<String>,
    pub affiliation: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JatsSection {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct JatsReference {
    pub doi: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JatsFigure {
    pub id: String,
    pub caption: String,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a JATS XML string into structured paper metadata.
///
/// Tolerant of missing elements — uses Option types and graceful fallbacks.
pub fn parse_jats(xml: &str) -> Result<JatsPaper, String> {
    let mut paper = JatsPaper::default();
    let mut reader = Reader::from_str(xml);

    // Stack tracks the nesting of elements we care about.
    let mut stack: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    // Temporary accumulators
    let mut current_author = JatsAuthor::default();
    let mut current_surname = String::new();
    let mut current_given = String::new();
    let mut current_section = JatsSection::default();
    let mut current_ref = JatsReference::default();
    let mut current_fig = JatsFigure::default();
    let mut in_abstract = false;
    let mut in_body = false;
    let mut in_ref_list = false;
    let mut in_kwd_group = false;
    let mut in_contrib = false;
    let mut in_fig = false;
    let mut in_section = false;
    let mut section_depth = 0u32;
    let mut abstract_section_depth = 0u32;
    let mut in_abstract_sec = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push(name.clone());

                match name.as_str() {
                    "abstract" => {
                        in_abstract = true;
                    }
                    "body" => {
                        in_body = true;
                    }
                    "ref-list" => {
                        in_ref_list = true;
                    }
                    "kwd-group" => {
                        in_kwd_group = true;
                    }
                    "contrib" => {
                        let is_author = e.attributes().filter_map(|a| a.ok()).any(|a| {
                            a.key.as_ref() == b"contrib-type" && a.value.as_ref() == b"author"
                        });
                        // Default to treating contrib as author if no type specified
                        if is_author
                            || !e
                                .attributes()
                                .filter_map(|a| a.ok())
                                .any(|a| a.key.as_ref() == b"contrib-type")
                        {
                            in_contrib = true;
                            current_author = JatsAuthor::default();
                            current_surname.clear();
                            current_given.clear();
                        }
                    }
                    "sec" if in_abstract => {
                        abstract_section_depth += 1;
                        in_abstract_sec = true;
                        current_section = JatsSection::default();
                    }
                    "sec" if in_body && !in_ref_list => {
                        section_depth += 1;
                        if section_depth == 1 {
                            in_section = true;
                            current_section = JatsSection::default();
                        }
                    }
                    "fig" | "table-wrap" => {
                        in_fig = true;
                        current_fig = JatsFigure::default();
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"id" {
                                current_fig.id = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "ref" if in_ref_list => {
                        current_ref = JatsReference::default();
                    }
                    "article-id" => {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"pub-id-type" {
                                let val = String::from_utf8_lossy(&attr.value).to_string();
                                // We'll capture the text in the Text event below
                                // Store a marker on the stack
                                if let Some(s) = stack.last_mut() {
                                    *s = format!("article-id:{}", val);
                                }
                            }
                        }
                    }
                    "contrib-id" if in_contrib => {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"contrib-id-type"
                                && attr.value.as_ref() == b"orcid"
                                && let Some(s) = stack.last_mut()
                            {
                                *s = "contrib-id:orcid".to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match name.as_str() {
                    "abstract" => {
                        in_abstract = false;
                    }
                    "body" => {
                        in_body = false;
                    }
                    "ref-list" => {
                        in_ref_list = false;
                    }
                    "kwd-group" => {
                        in_kwd_group = false;
                    }
                    "contrib" if in_contrib => {
                        in_contrib = false;
                        let full_name = match (current_given.is_empty(), current_surname.is_empty())
                        {
                            (false, false) => {
                                format!("{} {}", current_given.trim(), current_surname.trim())
                            }
                            (true, false) => current_surname.trim().to_string(),
                            (false, true) => current_given.trim().to_string(),
                            (true, true) => String::new(),
                        };
                        if !full_name.is_empty() {
                            current_author.name = full_name;
                            paper.authors.push(current_author.clone());
                        }
                    }
                    "sec" if in_abstract && in_abstract_sec => {
                        abstract_section_depth = abstract_section_depth.saturating_sub(1);
                        if abstract_section_depth == 0 {
                            in_abstract_sec = false;
                            let text = current_section.text.trim().to_string();
                            if !text.is_empty() {
                                current_section.text = text;
                                paper.abstract_sections.push(current_section.clone());
                            }
                        }
                    }
                    "sec" if in_body => {
                        section_depth = section_depth.saturating_sub(1);
                        if section_depth == 0 && in_section {
                            in_section = false;
                            let text = current_section.text.trim().to_string();
                            if !text.is_empty() {
                                current_section.text = text;
                                paper.body_sections.push(current_section.clone());
                            }
                        }
                    }
                    "fig" | "table-wrap" if in_fig => {
                        in_fig = false;
                        let caption = current_fig.caption.trim().to_string();
                        if !caption.is_empty() {
                            current_fig.caption = caption;
                            paper.figures.push(current_fig.clone());
                        }
                    }
                    "ref"
                        if in_ref_list
                            && (current_ref.doi.is_some() || current_ref.title.is_some()) =>
                    {
                        paper.references.push(current_ref.clone());
                    }
                    _ => {}
                }

                stack.pop();
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    buf.clear();
                    continue;
                }

                // Check what element we're inside
                let current = stack.last().map(|s| s.as_str()).unwrap_or("");

                match current {
                    "article-title" if !in_ref_list => {
                        if paper.title.is_empty() {
                            paper.title = trimmed.to_string();
                        }
                    }
                    s if s.starts_with("article-id:doi") => {
                        paper.doi = Some(trimmed.to_string());
                    }
                    s if s.starts_with("article-id:pmid") => {
                        paper.pmid = Some(trimmed.to_string());
                    }
                    s if s.starts_with("article-id:pmc") => {
                        paper.pmc = Some(trimmed.to_string());
                    }
                    "journal-title" => {
                        if paper.journal.is_none() {
                            paper.journal = Some(trimmed.to_string());
                        }
                    }
                    "year" => {
                        if paper.year.is_none()
                            && let Ok(y) = trimmed.parse::<i32>()
                        {
                            paper.year = Some(y);
                        }
                    }
                    "surname" if in_contrib => {
                        current_surname.push_str(trimmed);
                    }
                    "given-names" if in_contrib => {
                        current_given.push_str(trimmed);
                    }
                    "contrib-id:orcid" if in_contrib => {
                        let orcid = trimmed
                            .replace("https://orcid.org/", "")
                            .replace("http://orcid.org/", "");
                        current_author.orcid = Some(orcid);
                    }
                    "kwd" if in_kwd_group => {
                        paper.keywords.push(trimmed.to_string());
                    }
                    "title" if in_abstract && in_abstract_sec => {
                        current_section.title = trimmed.to_string();
                    }
                    "title" if in_body && in_section && section_depth == 1 => {
                        current_section.title = trimmed.to_string();
                    }
                    "article-title" if in_ref_list => {
                        current_ref.title = Some(trimmed.to_string());
                    }
                    _ => {
                        // Accumulate text for abstract, body sections, and figure captions
                        if in_abstract {
                            if !paper.abstract_text.is_empty()
                                && !paper.abstract_text.ends_with(' ')
                            {
                                paper.abstract_text.push(' ');
                            }
                            paper.abstract_text.push_str(trimmed);
                            if in_abstract_sec {
                                if !current_section.text.is_empty()
                                    && !current_section.text.ends_with(' ')
                                {
                                    current_section.text.push(' ');
                                }
                                current_section.text.push_str(trimmed);
                            }
                        }
                        if in_body && in_section && section_depth >= 1 {
                            if !current_section.text.is_empty()
                                && !current_section.text.ends_with(' ')
                            {
                                current_section.text.push(' ');
                            }
                            current_section.text.push_str(trimmed);
                        }
                        if in_fig && stack.iter().any(|s| s == "caption") {
                            if !current_fig.caption.is_empty()
                                && !current_fig.caption.ends_with(' ')
                            {
                                current_fig.caption.push(' ');
                            }
                            current_fig.caption.push_str(trimmed);
                        }
                        // Capture DOI in ref
                        if in_ref_list
                            && let Some(parent) = stack.iter().next_back()
                            && (parent == "pub-id" || parent.starts_with("pub-id"))
                        {
                            // Check if it looks like a DOI
                            if trimmed.starts_with("10.") || trimmed.contains('/') {
                                current_ref.doi = Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                // Self-closing elements like <contrib-id ... />
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "aff" && in_contrib {
                    // Sometimes affiliations are referenced via rid
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(format!(
                    "XML parse error at position {}: {e}",
                    reader.error_position()
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(paper)
}

// ---------------------------------------------------------------------------
// Conversion functions
// ---------------------------------------------------------------------------

/// Convert a parsed JATS paper into structured text suitable for LLM extraction.
///
/// Preserves section structure so the extractor can differentiate results from
/// discussion, and includes figure captions which often contain key quantitative claims.
#[allow(dead_code)]
pub fn jats_to_extraction_text(paper: &JatsPaper) -> String {
    let mut out = String::new();

    // Title
    out.push_str(&format!("TITLE: {}\n", paper.title));

    // Authors
    if !paper.authors.is_empty() {
        let author_strs: Vec<String> = paper
            .authors
            .iter()
            .map(|a| {
                if let Some(ref orcid) = a.orcid {
                    format!("{} ({})", a.name, orcid)
                } else {
                    a.name.clone()
                }
            })
            .collect();
        out.push_str(&format!("AUTHORS: {}\n", author_strs.join(", ")));
    }

    // Journal and year
    if let Some(ref journal) = paper.journal {
        let year_str = paper.year.map(|y| format!(" ({})", y)).unwrap_or_default();
        out.push_str(&format!("JOURNAL: {}{}\n", journal, year_str));
    }

    // DOI
    if let Some(ref doi) = paper.doi {
        out.push_str(&format!("DOI: {}\n", doi));
    }

    out.push('\n');

    // Abstract
    if !paper.abstract_sections.is_empty() {
        out.push_str("ABSTRACT:\n");
        for sec in &paper.abstract_sections {
            if !sec.title.is_empty() {
                out.push_str(&format!("{}: ", sec.title));
            }
            out.push_str(&sec.text);
            out.push('\n');
        }
    } else if !paper.abstract_text.is_empty() {
        out.push_str(&format!("ABSTRACT:\n{}\n", paper.abstract_text));
    }

    out.push('\n');

    // Body sections — map to canonical section names for the extractor
    for sec in &paper.body_sections {
        let title_upper = sec.title.to_uppercase();
        // Truncate long sections to keep within LLM context
        let max_chars = if title_upper.contains("METHOD") {
            500
        } else {
            2000
        };
        let text: String = sec.text.chars().take(max_chars).collect();

        if title_upper.contains("RESULT") {
            out.push_str(&format!("RESULTS:\n{}\n\n", text));
        } else if title_upper.contains("DISCUSSION") {
            out.push_str(&format!("DISCUSSION:\n{}\n\n", text));
        } else if title_upper.contains("METHOD") || title_upper.contains("MATERIAL") {
            out.push_str(&format!("METHODS:\n{}\n\n", text));
        } else if title_upper.contains("INTRODUCTION") || title_upper.contains("BACKGROUND") {
            // Skip introduction — not useful for extraction
        } else {
            out.push_str(&format!("{}:\n{}\n\n", sec.title.to_uppercase(), text));
        }
    }

    // Figure captions
    if !paper.figures.is_empty() {
        out.push_str("FIGURE CAPTIONS:\n");
        for (i, fig) in paper.figures.iter().enumerate() {
            let label = if fig.id.is_empty() {
                format!("Fig {}", i + 1)
            } else {
                fig.id.clone()
            };
            out.push_str(&format!("- {}: {}\n", label, fig.caption));
        }
    }

    out
}

/// Extract provenance metadata directly from JATS — no LLM needed.
pub fn jats_to_provenance(paper: &JatsPaper) -> Provenance {
    Provenance {
        source_type: "published_paper".to_string(),
        doi: paper.doi.clone(),
        pmid: paper.pmid.clone(),
        pmc: paper.pmc.clone(),
        openalex_id: None,
        url: None,
        title: paper.title.clone(),
        authors: paper
            .authors
            .iter()
            .map(|a| Author {
                name: a.name.clone(),
                orcid: a.orcid.clone(),
            })
            .collect(),
        year: paper.year,
        journal: paper.journal.clone(),
        license: None,
        publisher: None,
        funders: vec![],
        extraction: Extraction {
            method: "jats_ingestion".to_string(),
            model: None,
            model_version: None,
            extracted_at: String::new(), // filled in by caller
            extractor_version: "vela/0.2.0".to_string(),
        },
        review: None,
        citation_count: None,
    }
}

/// Convert a JatsPaper into a `fetch::Paper` for the existing extraction pipeline.
pub fn jats_to_paper(paper: &JatsPaper) -> crate::fetch::Paper {
    use crate::fetch::{Paper, PaperAuthor, PaperFullText};

    // Extract body sections into the PaperFullText format
    let mut results = String::new();
    let mut discussion = String::new();
    let mut methods = String::new();

    for sec in &paper.body_sections {
        let upper = sec.title.to_uppercase();
        if upper.contains("RESULT") {
            results.push_str(&sec.text);
            results.push(' ');
        } else if upper.contains("DISCUSSION") {
            discussion.push_str(&sec.text);
            discussion.push(' ');
        } else if upper.contains("METHOD") || upper.contains("MATERIAL") {
            methods.push_str(&sec.text);
            methods.push(' ');
        }
    }

    let figure_captions: Vec<String> = paper.figures.iter().map(|f| f.caption.clone()).collect();

    let full_text = if !results.is_empty() || !discussion.is_empty() {
        Some(PaperFullText {
            abstract_text: paper.abstract_text.clone(),
            results: results.trim().to_string(),
            discussion: discussion.trim().to_string(),
            methods: methods.trim().to_string(),
            figure_captions,
        })
    } else if !figure_captions.is_empty() || !methods.is_empty() {
        Some(PaperFullText {
            abstract_text: paper.abstract_text.clone(),
            results: String::new(),
            discussion: String::new(),
            methods: methods.trim().to_string(),
            figure_captions,
        })
    } else {
        None
    };

    Paper {
        title: paper.title.clone(),
        abstract_text: paper.abstract_text.clone(),
        doi: paper.doi.clone(),
        authors: paper
            .authors
            .iter()
            .map(|a| PaperAuthor {
                name: a.name.clone(),
                orcid: a.orcid.clone(),
            })
            .collect(),
        year: paper.year,
        citations: 0,
        openalex_id: None,
        full_text,
    }
}

/// Fetch JATS XML from PubMed Central by PMC ID.
pub async fn fetch_pmc_jats(client: &reqwest::Client, pmc_id: &str) -> Result<String, String> {
    // Strip "PMC" prefix if present to get the numeric part
    let id = pmc_id.strip_prefix("PMC").unwrap_or(pmc_id);
    let url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pmc&id={}&rettype=xml",
        id
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("PMC fetch error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("PMC returned HTTP {}", resp.status()));
    }

    resp.text()
        .await
        .map_err(|e| format!("PMC response body error: {e}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_JATS: &str = r#"<?xml version="1.0"?>
<article>
  <front>
    <journal-meta>
      <journal-title-group>
        <journal-title>Nature Neuroscience</journal-title>
      </journal-title-group>
    </journal-meta>
    <article-meta>
      <article-id pub-id-type="doi">10.1038/s41593-023-01234-5</article-id>
      <article-id pub-id-type="pmid">37654321</article-id>
      <article-id pub-id-type="pmc">PMC9876543</article-id>
      <title-group>
        <article-title>MFSD2A regulates blood-brain barrier transcytosis</article-title>
      </title-group>
      <contrib-group>
        <contrib contrib-type="author">
          <contrib-id contrib-id-type="orcid">https://orcid.org/0000-0001-2345-6789</contrib-id>
          <name>
            <surname>Zhang</surname>
            <given-names>Wei</given-names>
          </name>
        </contrib>
      </contrib-group>
      <pub-date pub-type="epub">
        <year>2023</year>
      </pub-date>
      <abstract>
        <p>MFSD2A is a lipid transporter essential for blood-brain barrier integrity.</p>
      </abstract>
      <kwd-group>
        <kwd>blood-brain barrier</kwd>
        <kwd>transcytosis</kwd>
      </kwd-group>
    </article-meta>
  </front>
</article>"#;

    const STRUCTURED_ABSTRACT_JATS: &str = r#"<?xml version="1.0"?>
<article>
  <front>
    <article-meta>
      <title-group>
        <article-title>RCT of Drug X in Alzheimer's Disease</article-title>
      </title-group>
      <abstract>
        <sec>
          <title>Background</title>
          <p>Alzheimer's disease affects millions worldwide.</p>
        </sec>
        <sec>
          <title>Methods</title>
          <p>We conducted a double-blind RCT with 500 participants.</p>
        </sec>
        <sec>
          <title>Results</title>
          <p>Drug X reduced amyloid plaque burden by 35% (p&lt;0.001).</p>
        </sec>
        <sec>
          <title>Conclusions</title>
          <p>Drug X shows significant amyloid reduction in early AD.</p>
        </sec>
      </abstract>
    </article-meta>
  </front>
</article>"#;

    const FULL_PAPER_JATS: &str = r#"<?xml version="1.0"?>
<article>
  <front>
    <journal-meta>
      <journal-title-group>
        <journal-title>Cell</journal-title>
      </journal-title-group>
    </journal-meta>
    <article-meta>
      <article-id pub-id-type="doi">10.1016/j.cell.2024.01.001</article-id>
      <title-group>
        <article-title>NLRP3 inflammasome in neurodegeneration</article-title>
      </title-group>
      <contrib-group>
        <contrib contrib-type="author">
          <name>
            <surname>Smith</surname>
            <given-names>John A.</given-names>
          </name>
        </contrib>
        <contrib contrib-type="author">
          <contrib-id contrib-id-type="orcid">0000-0002-3456-7890</contrib-id>
          <name>
            <surname>Lee</surname>
            <given-names>Min</given-names>
          </name>
        </contrib>
      </contrib-group>
      <pub-date pub-type="epub">
        <year>2024</year>
      </pub-date>
      <abstract>
        <p>The NLRP3 inflammasome drives neuroinflammation in AD and PD.</p>
      </abstract>
    </article-meta>
  </front>
  <body>
    <sec>
      <title>Introduction</title>
      <p>Neuroinflammation is a key driver of neurodegeneration.</p>
    </sec>
    <sec>
      <title>Results</title>
      <p>NLRP3 knockout mice showed 60% reduction in tau phosphorylation.</p>
      <p>IL-1B levels were decreased by 45% in treated animals.</p>
      <fig id="fig1">
        <caption><p>Figure 1. NLRP3 knockout reduces tau pathology in 5xFAD mice.</p></caption>
      </fig>
    </sec>
    <sec>
      <title>Discussion</title>
      <p>Our findings demonstrate that NLRP3 is a viable therapeutic target.</p>
    </sec>
    <sec>
      <title>Methods</title>
      <p>5xFAD mice were crossed with NLRP3 knockout mice and analyzed at 6 months.</p>
    </sec>
  </body>
  <back>
    <ref-list>
      <ref>
        <element-citation>
          <article-title>Inflammasomes in neurological diseases</article-title>
          <pub-id pub-id-type="doi">10.1038/nri.2021.42</pub-id>
        </element-citation>
      </ref>
    </ref-list>
  </back>
</article>"#;

    #[test]
    fn parse_minimal_jats() {
        let paper = parse_jats(MINIMAL_JATS).unwrap();
        assert_eq!(
            paper.title,
            "MFSD2A regulates blood-brain barrier transcytosis"
        );
        assert_eq!(paper.doi.as_deref(), Some("10.1038/s41593-023-01234-5"));
        assert_eq!(paper.pmid.as_deref(), Some("37654321"));
        assert_eq!(paper.pmc.as_deref(), Some("PMC9876543"));
        assert_eq!(paper.year, Some(2023));
        assert_eq!(paper.journal.as_deref(), Some("Nature Neuroscience"));
        assert_eq!(paper.authors.len(), 1);
        assert_eq!(paper.authors[0].name, "Wei Zhang");
        assert_eq!(
            paper.authors[0].orcid.as_deref(),
            Some("0000-0001-2345-6789")
        );
        assert!(paper.abstract_text.contains("MFSD2A"));
        assert_eq!(paper.keywords.len(), 2);
        assert!(paper.keywords.contains(&"blood-brain barrier".to_string()));
    }

    #[test]
    fn parse_structured_abstract() {
        let paper = parse_jats(STRUCTURED_ABSTRACT_JATS).unwrap();
        assert_eq!(paper.title, "RCT of Drug X in Alzheimer's Disease");
        assert!(!paper.abstract_sections.is_empty());
        // Should have captured section titles
        let titles: Vec<&str> = paper
            .abstract_sections
            .iter()
            .map(|s| s.title.as_str())
            .collect();
        assert!(titles.contains(&"Background"));
        assert!(titles.contains(&"Results"));
        // Full abstract text should contain content from all sections
        assert!(paper.abstract_text.contains("Alzheimer"));
        assert!(paper.abstract_text.contains("amyloid"));
    }

    #[test]
    fn parse_missing_fields() {
        let minimal = r#"<?xml version="1.0"?>
<article>
  <front>
    <article-meta>
      <title-group>
        <article-title>Sparse Paper</article-title>
      </title-group>
    </article-meta>
  </front>
</article>"#;
        let paper = parse_jats(minimal).unwrap();
        assert_eq!(paper.title, "Sparse Paper");
        assert!(paper.doi.is_none());
        assert!(paper.pmid.is_none());
        assert!(paper.authors.is_empty());
        assert!(paper.year.is_none());
        assert!(paper.journal.is_none());
        assert!(paper.keywords.is_empty());
        assert!(paper.body_sections.is_empty());
    }

    #[test]
    fn extraction_text_format() {
        let paper = parse_jats(FULL_PAPER_JATS).unwrap();
        let text = jats_to_extraction_text(&paper);

        assert!(text.starts_with("TITLE: NLRP3 inflammasome"));
        assert!(text.contains("AUTHORS:"));
        assert!(text.contains("John A. Smith"));
        assert!(text.contains("Min Lee"));
        assert!(text.contains("0000-0002-3456-7890"));
        assert!(text.contains("JOURNAL: Cell (2024)"));
        assert!(text.contains("DOI: 10.1016/j.cell.2024.01.001"));
        assert!(text.contains("ABSTRACT:"));
        assert!(text.contains("RESULTS:"));
        assert!(text.contains("tau phosphorylation"));
        assert!(text.contains("DISCUSSION:"));
        assert!(text.contains("METHODS:"));
        assert!(text.contains("FIGURE CAPTIONS:"));
        assert!(text.contains("NLRP3 knockout reduces tau pathology"));
    }

    #[test]
    fn provenance_extraction() {
        let paper = parse_jats(FULL_PAPER_JATS).unwrap();
        let prov = jats_to_provenance(&paper);

        assert_eq!(prov.source_type, "published_paper");
        assert_eq!(prov.doi.as_deref(), Some("10.1016/j.cell.2024.01.001"));
        assert_eq!(prov.title, "NLRP3 inflammasome in neurodegeneration");
        assert_eq!(prov.year, Some(2024));
        assert_eq!(prov.journal.as_deref(), Some("Cell"));
        assert_eq!(prov.authors.len(), 2);
        assert_eq!(prov.authors[0].name, "John A. Smith");
        assert!(prov.authors[0].orcid.is_none());
        assert_eq!(prov.authors[1].name, "Min Lee");
        assert_eq!(
            prov.authors[1].orcid.as_deref(),
            Some("0000-0002-3456-7890")
        );
        assert_eq!(prov.extraction.method, "jats_ingestion");
    }

    #[test]
    fn full_paper_body_sections() {
        let paper = parse_jats(FULL_PAPER_JATS).unwrap();
        assert!(paper.body_sections.len() >= 3);
        let section_titles: Vec<&str> = paper
            .body_sections
            .iter()
            .map(|s| s.title.as_str())
            .collect();
        assert!(section_titles.contains(&"Results"));
        assert!(section_titles.contains(&"Discussion"));
        assert!(section_titles.contains(&"Methods"));
    }

    #[test]
    fn full_paper_references() {
        let paper = parse_jats(FULL_PAPER_JATS).unwrap();
        assert_eq!(paper.references.len(), 1);
        assert_eq!(
            paper.references[0].title.as_deref(),
            Some("Inflammasomes in neurological diseases")
        );
        assert_eq!(
            paper.references[0].doi.as_deref(),
            Some("10.1038/nri.2021.42")
        );
    }

    #[test]
    fn full_paper_figures() {
        let paper = parse_jats(FULL_PAPER_JATS).unwrap();
        assert_eq!(paper.figures.len(), 1);
        assert_eq!(paper.figures[0].id, "fig1");
        assert!(paper.figures[0].caption.contains("NLRP3 knockout"));
    }

    #[test]
    fn jats_to_paper_conversion() {
        let jats_paper = parse_jats(FULL_PAPER_JATS).unwrap();
        let paper = jats_to_paper(&jats_paper);

        assert_eq!(paper.title, "NLRP3 inflammasome in neurodegeneration");
        assert_eq!(paper.doi.as_deref(), Some("10.1016/j.cell.2024.01.001"));
        assert_eq!(paper.authors.len(), 2);
        assert!(paper.full_text.is_some());
        let ft = paper.full_text.unwrap();
        assert!(ft.results.contains("tau phosphorylation"));
        assert!(ft.discussion.contains("therapeutic target"));
        assert_eq!(ft.figure_captions.len(), 1);
    }
}
