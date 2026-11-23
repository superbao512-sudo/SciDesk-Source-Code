export type PaperFeedItem = {
  pushItemId: string; paperId: string; topicId: string;
  title: string; abstractText: string | null; abstractStatus: string;
  authors: string[]; paperType: 'journal_article'|'preprint'; source: 'arxiv'|'crossref';
  journalName: string | null; journalShortName?: string | null; journalIcon: string | null;
  publishedAt: string | null; firstSeenAt: string;
  doi: string | null; arxivId: string | null; articleUrl: string; arxivUrl: string | null;
  score: number; relevanceLevel: 'high'|'medium'; matchedTerms: unknown[]; algorithmVersion: string;
  status: 'unread'|'read'|'dismissed'; createdAt: string;
};

export type PaperFeedListOptions = {
  topicId?: string; journalId?: string; paperId?: string; relevance?: 'high'|'medium'; source?: 'arxiv'|'crossref';
  paperType?: 'journal_article'|'preprint'; status?: 'unread'|'read'|'dismissed';
  limit?: number; cursor?: string; search?: string; sort?: 'time'|'relevance';
};
export type PaperFeedPage = {items: PaperFeedItem[]; total: number; unreadCount: number; nextCursor: string | null};
export type PaperFeedStats = {total: number; unread: number; high: number; medium: number; journalArticles: number; preprints: number};
export type PaperFeedStatus = 'unread'|'read'|'dismissed';

export type PaperTopicKeyword = {phrase:string; weight:number; field:'title'|'abstract'|'both'; kind:'required'|'boost'|'exclude'; enabled:boolean; notes:string};
export type PaperTopic = {id:string; name:string; description:string; enabled:boolean; matchLogic:'all'|'any'; matchAlgorithmVersion:string; createdAt:string; updatedAt:string; keywords:PaperTopicKeyword[]};
export type PaperTopicInput = {name:string; description?:string; enabled?:boolean; matchLogic?:'all'|'any'; keywords: Array<{phrase:string; weight:number; field:'title'|'abstract'|'both'; kind:'required'|'boost'|'exclude'; enabled?:boolean; notes?:string}>};
export type Catalog = {topics: Array<{id:string; name:string; matchCount:number; enabled:boolean}>; journals: Array<{id:string; name:string; shortName:string; articleCount:number}>};
/** Per-journal Crossref synchronization state returned by the Rust boundary. */
export type PaperJournalSubscription = {
  id:string;
  enabled:boolean;
  syncEnabled:boolean;
  lastStatus:string;
  lastSuccessAt?:string|null;
  lastAttemptAt?:string|null;
  lastError?:string|null;
  lastErrorType?:string|null;
  lastQueryCount?:number;
  lastFetchedCount?:number;
  lastParsedCount?:number;
  lastInsertedCount?:number;
  lastUpdatedCount?:number;
  lastMatchedCount?:number;
  lastPushedCount?:number;
};
export type PaperJournal = {id:string; name:string;series:string; shortName:string; issnPrint:string|null; issnOnline:string|null; publisher:string|null; websiteUrl:string|null; subscription:PaperJournalSubscription|null; topicIds:string[]; createdAt:string; updatedAt:string};
export type PaperJournalInput = {name:string;series?:string; shortName?:string; issnPrint?:string; issnOnline?:string; publisher?:string; websiteUrl?:string};

export type SyncRun={source:string;status:string;queries:number;fetched:number;parsed:number;inserted:number;updated:number;matched:number;pushed:number;error:string|null};

export type LibraryJournal={id:string;name:string;shortName:string;issn:string;publisher:string;websiteUrl:string;group:string;source:string};
export type ScholarProfile={url:string|null;snapshot:{source?:string;name:string;affiliation:string;interests:string[];works:{title:string;authors:string;publication:string;year:number|null}[]}|null;lastAttemptAt:string|null;lastSuccessAt:string|null;error:string|null};

export type PaperSourceStatus={id:string;name:string;websiteUrl:string;subscription:PaperJournalSubscription};
