$if(has-meta)$---
$if(title)$title: "$title$"
$endif$$if(author-single)$author: "$author-single$"
$endif$$if(author-list)$author:
$for(author-list)$$if(author-list.has-details)$  - name: "$author-list.name$"
$if(author-list.affiliation)$    affiliation: "$author-list.affiliation$"
$endif$$if(author-list.email)$    email: "$author-list.email$"
$endif$$if(author-list.orcid)$    orcid: "$author-list.orcid$"
$endif$$else$  - "$author-list.name$"
$endif$$endfor$$endif$$if(date)$date: "$date$"
$endif$$if(abstract)$abstract: "$abstract$"
$endif$$if(keywords)$keywords: [$for(keyword)$"$keyword$"$sep$, $endfor$]
$endif$$if(custom-meta)$$custom-meta$$endif$---

$endif$$body$
