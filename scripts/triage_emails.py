#!/usr/bin/env python3
"""
Email triage script - categorizes emails based on sender domain
Add domains to ARCHIVE_SENDERS as we confirm they're archivable
"""
import json
import os
import shutil
from pathlib import Path
from collections import defaultdict

INBOX = Path("emails/inbox")
TO_ARCHIVE = Path("emails/to_archive")
NEEDS_REVIEW = Path("emails/needs_review")

# Senders to always archive - add domains here as we confirm they're noise
ARCHIVE_SENDERS = {
    "honeybirdette.com",
    "e.inc.com",
    "service.tiktok.com",
    "a.grubhub.com",
    "ocasiocortez.com",
    "e4.rover.com",
    "petfood.express",
    # batch 2
    "substack.com",
    "piratewires.com",
    "email.nanit.com",
    "news.thebump.com",
    "email.chipotle.com",
    "mail.toogoodtogo.com",
    "defaultkings.com",
    "thetiebar.com",
    "listi.jpberlin.de",
    # batch 3 - notifications
    "voice-noreply@google.com",
    "notifications@bugsnag.com",
    "squaremktg.com",
    "txt.voice.google.com",
    # batch 4 - more notifications
    "noreply@bambulab.com",
    "gusto.com",
    "inform.bill.com",
    "no-reply@digikey.com",
    "notification.intuit.com",
    "NoReply.ODD@dhl.com",
    "no-reply-aws@amazon.com",
    "cloudplatform-noreply@google.com",
    "no-reply@accounts.google.com",
    "workspace-noreply@google.com",
    "no-reply@amazon.com",
    "no-reply@patreon.com",
    "no-reply@mailchimp.com",
    "donotreply@appfolio.com",
    "donotreply@onlineportal.appfolio.com",
    "members.mobilize.io",
    "catalystcampus.org",
    "info@prusa3d.com",
    "payments-noreply@google.com",
    "stellerarts.com",
    "drive-shares-dm-noreply@google.com",
    # batch 5 - more notifications/marketing
    "simon-mail.groometransportation.com",
    "noreply@qemailserver.com",
    "notifications.intuit.com",
    "email.slackhq.com",
    "adeointeractive.com",
    "cps.iau.org",
    "email.claude.com",
    # batch 6 - newsletters/marketing
    "info@printables.com",
    "qualtrics-research.com",
    "Publications.spie.org",
    "info@swfound.org",
    "no-reply@login.gov",
    "drangkromeet.com",
    # batch 7 - old newsletters/lists
    "secondtimefounders.com",
    "trwih.com",
    "barbaraleeforcongress.org",
    # batch 8 - marketing spam
    "c.hellofresh.com",
    "email.ticketmaster.com",
    "roktpowered.com",
    "101domain.com",
    "simonsfoundation.org",
    "slack.zendesk.com",
    "contact.exploratorium.edu",
    # batch 9 - political
    "yimbyaction.org",
    "e.giffords.org",
    "e.votevets.org",
    "e.elissaslotkin.org",
    "e.chrispappas.org",
    "foe.org",
    # batch 10 - more newsletters/marketing
    "mail.hallow.com",
    "e.roadtrippers.com",
    "e.starbucks.com",
    "newsletter.trip.com",
    "23andme.com",
    "bookcrossing.com",
    "peakdesign.com",
    "bark.co",
    "emails.wyndhamhotels.com",
    "fluent.pet",
    "mailout.plex.tv",
    "ctoconnection.com",
    "chewy.com",
    "e.vacasa.com",
    "join.netflix.com",
    "e.worldnomads.com",
    "tonal.com",
    "mschf.com",
    "announcements.soundcloud.com",
    "updates.miro.com",
    "info.haymarketmedicalnetwork.com",
    # batch 11 - more marketing/notifications
    "baukunst.co",
    "latecheckout.studio",
    "notifications.ro.co",
    "thelostchurch.org",
    "nanit.com",
    "namecheap.com",
    "autodeskcommunications.com",
    "sensibo.com",
    "e.electjon.com",
    "techcrunch.com",
    "audible.com",
    "vcita.com",
    "email.1password.com",
    "mail.coinbase.com",
    "twitch.tv",
    # batch 12 - more marketing
    "slack.com",
    "mail.airtable.com",
    "mn.co",
    "thekla.com",
    "messaging.squareup.com",
    "butterand.com",
    "androidjones.com",
    "babalou.com",
    "craigslist.org",
    "reply.craigslist.org",
    "gregangelo.com",
    "texasexesemail.com",
    "proofre.com",
    "qrzcq.com",
    "barszranch.com",
    "e.smartnews.com",
    "theclymb.com",
    "is.email.nextdoor.com",
    "reply.chargepoint.com",
    "team.twilio.com",
    "e.debhaaland.com",
    "rackwarehouse.com",
    "paws.petmeds.com",
    "kochdavisjobs.com",
    "fightforthefuture.org",
    "youtube.com",
    "outreach.assembly.ca.gov",
    # batch 13 - more marketing/notifications
    "catmachin.com",
    "info.blueshieldca.com",
    "mailgun.gingrapp.com",
    "connect.othership.us",
    "polytrope.com",
    "greathighwaypark.com",
    "linkedin.com",
    "redheron.com",
    "labcorpmessage.com",
    "email.modernanimal.com",
    "e.healio.com",
    "email.classmates.com",
    "false-profit.com",
    "heathceramics.com",
    "vowel.com",
    "email.venmo.com",
    "wakethetalkup.com",
    "engineeredartworks.com",
    "duolingo.com",
    "em.shutterfly.com",
    "vagaro.com",
    "retainhr.com",
    # batch 14 - more marketing/notifications
    "marketing.coveredca.com",
    "e.simonmed.com",
    "g.hellofresh.com",
    "visaprepaidprocessing.com",
    "service.alibaba.com",
    "engage.microsoft.com",
    "notices.rei.com",
    "godaddy.com",
    "frankenforiowa.org",
    "flocksafety.com",
    "unity3d.com",
    "dnuke.com",
    "avimark1.com",
    "sagebionetworks.org",
    "spaero.bio",
    # batch 15 - more marketing/newsletters
    "mealtrain.com",
    "campaign.eventbrite.com",
    "g.kajabimail.net",
    "c.kajabimail.net",
    "mrktgllry.com",
    "globaldatamarketplace.com",
    "me.kickstarter.com",
    "joshshapiro.org",
    "mail.keeps.com",
    "mail.lakeshorelearning.com",
    "fogcitydogs.com",
    "e.fastcompany.com",
    "onemedical.com",
    "honeyhomes.com",
    # batch 16 - more marketing/newsletters
    "lookingup.art",
    "climatetechaction.network",
    "crunchlabs.com",
    "producthunt.com",
    "mail.adobe.com",
    "173366937.mailchimpapp.com",
    "kimblegroup.com",
    # batch 17 - community/retail
    "hazardfactory.org",
    "ss.email.nextdoor.com",
    "sportsbasement.com",
    "onlinebookclub.org",
    "manifold.markets",
}

# Senders that should go to review (potentially actionable)
REVIEW_SENDERS = {
    # Add senders that need human attention here
}

def should_archive(email):
    """Return True if email should be auto-archived"""
    from_addr = email.get("from", "").lower()
    for pattern in ARCHIVE_SENDERS:
        if pattern in from_addr:
            return True
    return False

def needs_review(email):
    """Return True if email might need attention"""
    from_addr = email.get("from", "").lower()
    for pattern in REVIEW_SENDERS:
        if pattern in from_addr:
            return True
    return False

def get_domain(from_addr):
    """Extract domain from email address"""
    if "<" in from_addr:
        from_addr = from_addr.split("<")[1].split(">")[0]
    if "@" in from_addr:
        return from_addr.split("@")[1].lower()
    return from_addr.lower()[:50]

def main():
    TO_ARCHIVE.mkdir(exist_ok=True)
    NEEDS_REVIEW.mkdir(exist_ok=True)

    stats = {"archive": 0, "review": 0, "unknown": 0}
    sender_counts = defaultdict(int)

    for filepath in sorted(INBOX.glob("*.json")):
        try:
            with open(filepath) as f:
                email = json.load(f)
        except:
            continue

        if needs_review(email):
            shutil.move(str(filepath), str(NEEDS_REVIEW / filepath.name))
            stats["review"] += 1
        elif should_archive(email):
            shutil.move(str(filepath), str(TO_ARCHIVE / filepath.name))
            stats["archive"] += 1
        else:
            stats["unknown"] += 1
            domain = get_domain(email.get("from", "unknown"))
            sender_counts[domain] += 1

    print(f"Archived:     {stats['archive']}")
    print(f"To Review:    {stats['review']}")
    print(f"Unprocessed:  {stats['unknown']}")

    print("\n=== SENDER DOMAINS BY COUNT (add to ARCHIVE_SENDERS to archive) ===")
    sorted_senders = sorted(sender_counts.items(), key=lambda x: -x[1])
    for domain, count in sorted_senders[:60]:
        print(f"  {count:4d}  {domain}")

if __name__ == "__main__":
    main()
