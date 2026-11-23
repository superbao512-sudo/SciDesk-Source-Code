UPDATE idea_records SET data=json_set(data,'$.source_anchor','right','$.target_anchor','left','$.line_style','solid','$.color','#8199ba') WHERE kind='edge';
PRAGMA user_version=3;
